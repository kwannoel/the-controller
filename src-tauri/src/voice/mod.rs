pub mod audio_input;
pub mod audio_output;
pub mod gain;
pub mod llm;
pub mod models;
pub mod stt;
pub mod tts;
pub mod vad;

use crate::emitter::EventEmitter;
use crossbeam_channel::{Receiver, Sender};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VoiceState {
    Listening,
    Thinking,
    Speaking,
    Downloading,
}

#[derive(Serialize)]
struct VoiceStateEvent {
    state: VoiceState,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    percent: Option<u8>,
}

const MIN_SPEECH_SAMPLES: usize = 8000; // 0.5s at 16kHz
const MIN_SPEECH_RMS: f32 = 0.02;

pub struct VoicePipeline {
    stop_flag: Arc<AtomicBool>,
    audio_thread: Option<std::thread::JoinHandle<()>>,
}

impl VoicePipeline {
    /// Start the voice pipeline. Downloads models if needed, then begins listening.
    pub async fn start(emitter: Arc<dyn EventEmitter>) -> Result<Self, String> {
        let stop_flag = Arc::new(AtomicBool::new(false));

        // Ensure models are downloaded
        let dl_emitter = emitter.clone();
        let model_paths = models::ensure_models(|filename, downloaded, total| {
            let percent = total.map(|t| if t > 0 { ((downloaded * 100) / t).min(100) as u8 } else { 0 });
            let payload = serde_json::to_string(&VoiceStateEvent {
                state: VoiceState::Downloading,
                filename: Some(filename.to_string()),
                percent,
            })
            .unwrap_or_default();
            let _ = dl_emitter.emit("voice-state-changed", &payload);
        })
        .await?;

        let stop = stop_flag.clone();
        let emitter_clone = emitter.clone();

        let vad_path = model_paths.silero_vad.clone();
        let whisper_path = model_paths.whisper.clone();
        let piper_onnx_path = model_paths.piper_onnx.clone();
        let piper_config_path = model_paths.piper_config.clone();

        let audio_thread = std::thread::spawn(move || {
            if let Err(e) = run_pipeline(
                &vad_path,
                &whisper_path,
                &piper_onnx_path,
                &piper_config_path,
                stop,
                emitter_clone.clone(),
            ) {
                eprintln!("[voice] Pipeline error: {e}");
                let payload = serde_json::json!({
                    "state": "error",
                    "error": e,
                })
                .to_string();
                let _ = emitter_clone.emit("voice-state-changed", &payload);
            }
        });

        Ok(Self {
            stop_flag,
            audio_thread: Some(audio_thread),
        })
    }

    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(handle) = self.audio_thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for VoicePipeline {
    fn drop(&mut self) {
        self.stop();
    }
}

fn emit_state(emitter: &Arc<dyn EventEmitter>, state: VoiceState) {
    let payload = serde_json::to_string(&VoiceStateEvent {
        state,
        filename: None,
        percent: None,
    })
    .unwrap_or_default();
    let _ = emitter.emit("voice-state-changed", &payload);
}

fn emit_debug(emitter: &Arc<dyn EventEmitter>, msg: &str) {
    let payload = serde_json::json!({
        "ts": chrono::Local::now().format("%H:%M:%S").to_string(),
        "msg": msg,
    })
    .to_string();
    let _ = emitter.emit("voice-debug", &payload);
}

/// Main pipeline loop. Runs on a dedicated thread.
fn run_pipeline(
    vad_path: &std::path::Path,
    whisper_path: &std::path::Path,
    piper_onnx_path: &std::path::Path,
    piper_config_path: &std::path::Path,
    stop: Arc<AtomicBool>,
    emitter: Arc<dyn EventEmitter>,
) -> Result<(), String> {
    // Initialize components
    let mut vad_engine = vad::Vad::new(vad_path, 800)?;
    let whisper = stt::WhisperStt::new(whisper_path)?;
    let mut tts_engine = tts::PiperTts::new(piper_onnx_path, piper_config_path)?;
    let audio_out = audio_output::AudioOutput::new()?;
    let mut auto_gain = gain::AutoGain::new();
    let mut conversation = llm::Conversation::new(None);

    // Start mic capture
    let (tx, rx): (Sender<Vec<i16>>, Receiver<Vec<i16>>) = crossbeam_channel::bounded(64);
    let mut audio_in = audio_input::AudioInput::start(tx)?;

    emit_state(&emitter, VoiceState::Listening);
    let mut speech_buffer: Vec<f32> = Vec::new();
    let mut in_speech = false;
    let mut chunk_count: u64 = 0;

    while !stop.load(Ordering::Relaxed) {
        let chunk = match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(c) => c,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        };

        let normalized = auto_gain.apply(&chunk);

        chunk_count += 1;
        if chunk_count % 15 == 0 {
            let rms_i16 = (chunk.iter().map(|&s| (s as f32) * (s as f32)).sum::<f32>()
                / chunk.len() as f32)
                .sqrt();
            let prob = vad_engine.last_prob();
            emit_debug(&emitter, &format!("mic rms={:.0} vad prob={:.3}", rms_i16, prob));
        }

        match vad_engine.process(&normalized)? {
            Some(vad::VadEvent::SpeechStart) => {
                emit_debug(&emitter, "speech_start");
                in_speech = true;
                speech_buffer.clear();
                speech_buffer.extend_from_slice(&normalized);
            }
            Some(vad::VadEvent::SpeechEnd) => {
                if in_speech {
                    speech_buffer.extend_from_slice(&normalized);
                    in_speech = false;
                    emit_debug(&emitter, &format!("speech_end ({:.1}s)", speech_buffer.len() as f32 / 16000.0));

                    process_speech(
                        &speech_buffer,
                        &whisper,
                        &mut tts_engine,
                        &audio_out,
                        &mut audio_in,
                        &mut conversation,
                        &mut vad_engine,
                        &emitter,
                        &stop,
                    )?;

                    speech_buffer.clear();
                    if !stop.load(Ordering::Relaxed) {
                        emit_state(&emitter, VoiceState::Listening);
                    }
                }
            }
            None => {
                if in_speech {
                    speech_buffer.extend_from_slice(&normalized);
                }
            }
        }
    }

    audio_in.stop();
    Ok(())
}

fn process_speech(
    audio: &[f32],
    whisper: &stt::WhisperStt,
    tts_engine: &mut tts::PiperTts,
    audio_out: &audio_output::AudioOutput,
    audio_in: &mut audio_input::AudioInput,
    conversation: &mut llm::Conversation,
    vad_engine: &mut vad::Vad,
    emitter: &Arc<dyn EventEmitter>,
    stop: &Arc<AtomicBool>,
) -> Result<(), String> {
    if audio.len() < MIN_SPEECH_SAMPLES {
        return Ok(());
    }

    let rms = (audio.iter().map(|&s| s * s).sum::<f32>() / audio.len() as f32).sqrt();
    if rms < MIN_SPEECH_RMS {
        return Ok(());
    }

    // STT
    emit_state(emitter, VoiceState::Thinking);
    let text = whisper.transcribe(audio)?;
    if text.is_empty() {
        return Ok(());
    }

    eprintln!("[voice] You: {text}");
    conversation.add_user(&text);
    emit_debug(emitter, &format!("stt: \"{}\"", text));
    let _ = emitter.emit("voice-transcript", &serde_json::json!({"role": "user", "text": text}).to_string());

    // LLM — need a tokio runtime since we're on a std thread
    emit_debug(emitter, "llm: streaming...");
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("Failed to create tokio runtime: {e}"))?;

    let response = rt.block_on(async {
        let mut full = String::new();
        llm::stream_response(conversation, &mut |token| {
            full.push_str(token);
        })
        .await
        .map(|_| full)
    })?;

    if response.is_empty() || stop.load(Ordering::Relaxed) {
        return Ok(());
    }

    eprintln!("[voice] Assistant: {response}");
    conversation.add_assistant(&response);
    emit_debug(emitter, "llm: done");
    let _ = emitter.emit("voice-transcript", &serde_json::json!({"role": "assistant", "text": response}).to_string());

    // TTS
    emit_state(emitter, VoiceState::Speaking);
    emit_debug(emitter, "tts: synthesizing...");
    audio_in.mute();

    let tts_sample_rate = tts_engine.sample_rate();
    for chunk_result in tts_engine.synthesize_streaming(&response) {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        match chunk_result {
            Ok(samples) if !samples.is_empty() => {
                audio_out.play_i16(&samples, tts_sample_rate)?;
            }
            Err(e) => eprintln!("[voice] TTS error: {e}"),
            _ => {}
        }
    }

    audio_in.unmute();
    emit_debug(emitter, "tts: done");
    std::thread::sleep(std::time::Duration::from_millis(300));
    vad_engine.reset();

    Ok(())
}
