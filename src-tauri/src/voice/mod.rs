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

struct SpeechContext<'a> {
    whisper: &'a stt::WhisperStt,
    tts_engine: &'a mut tts::PiperTts,
    audio_out: &'a audio_output::AudioOutput,
    audio_in: &'a mut audio_input::AudioInput,
    conversation: &'a mut llm::Conversation,
    vad_engine: &'a mut vad::Vad,
    emitter: &'a Arc<dyn EventEmitter>,
    stop: &'a Arc<AtomicBool>,
}

impl VoicePipeline {
    /// Start the voice pipeline. Downloads models if needed, then begins listening.
    pub async fn start(emitter: Arc<dyn EventEmitter>) -> Result<Self, String> {
        let stop_flag = Arc::new(AtomicBool::new(false));

        // Ensure models are downloaded
        let dl_emitter = emitter.clone();
        let model_paths = models::ensure_models(|filename, downloaded, total| {
            let percent = total.map(|t| {
                if t > 0 {
                    ((downloaded * 100) / t).min(100) as u8
                } else {
                    0
                }
            });
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
        if chunk_count.is_multiple_of(15) {
            let rms_i16 = (chunk.iter().map(|&s| (s as f32) * (s as f32)).sum::<f32>()
                / chunk.len() as f32)
                .sqrt();
            let prob = vad_engine.last_prob();
            emit_debug(
                &emitter,
                &format!("mic rms={:.0} vad prob={:.3}", rms_i16, prob),
            );
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
                    emit_debug(
                        &emitter,
                        &format!("speech_end ({:.1}s)", speech_buffer.len() as f32 / 16000.0),
                    );

                    let mut speech_ctx = SpeechContext {
                        whisper: &whisper,
                        tts_engine: &mut tts_engine,
                        audio_out: &audio_out,
                        audio_in: &mut audio_in,
                        conversation: &mut conversation,
                        vad_engine: &mut vad_engine,
                        emitter: &emitter,
                        stop: &stop,
                    };

                    process_speech(&speech_buffer, &mut speech_ctx)?;

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

fn process_speech(audio: &[f32], ctx: &mut SpeechContext<'_>) -> Result<(), String> {
    if audio.len() < MIN_SPEECH_SAMPLES {
        return Ok(());
    }

    let rms = (audio.iter().map(|&s| s * s).sum::<f32>() / audio.len() as f32).sqrt();
    if rms < MIN_SPEECH_RMS {
        return Ok(());
    }

    // STT
    emit_state(ctx.emitter, VoiceState::Thinking);
    let text = ctx.whisper.transcribe(audio)?;
    if text.is_empty() {
        return Ok(());
    }

    eprintln!("[voice] You: {text}");
    ctx.conversation.add_user(&text);
    emit_debug(ctx.emitter, &format!("stt: \"{}\"", text));
    let _ = ctx.emitter.emit(
        "voice-transcript",
        &serde_json::json!({"role": "user", "text": text}).to_string(),
    );

    // Stream LLM → TTS → Audio concurrently.
    // LLM runs in a background thread, splitting tokens into sentences.
    // Main thread synthesizes each sentence via TTS and streams audio immediately.
    emit_debug(ctx.emitter, "llm: streaming...");

    let conv_clone = ctx.conversation.clone();
    let emitter_for_llm = ctx.emitter.clone();
    let (sentence_tx, sentence_rx) = crossbeam_channel::bounded::<String>(8);

    let llm_handle = std::thread::spawn(move || -> Result<String, String> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create tokio runtime: {e}"))?;
        rt.block_on(async {
            let mut sentence_buf = String::new();
            let mut full_response = String::new();
            llm::stream_response(&conv_clone, &mut |token| {
                sentence_buf.push_str(token);
                full_response.push_str(token);
                // Send complete sentences as they form
                while let Some(pos) =
                    sentence_buf.find(|c: char| matches!(c, '.' | '!' | '?'))
                {
                    let sentence = sentence_buf[..=pos].trim().to_string();
                    sentence_buf = sentence_buf[pos + 1..].to_string();
                    if !sentence.is_empty() {
                        let _ = sentence_tx.send(sentence);
                    }
                }
            })
            .await?;
            // Flush remaining text (no trailing punctuation)
            let remaining = sentence_buf.trim().to_string();
            if !remaining.is_empty() {
                let _ = sentence_tx.send(remaining);
            }
            // Emit transcript as soon as LLM finishes (before audio finishes)
            if !full_response.is_empty() {
                eprintln!("[voice] Assistant: {full_response}");
                emit_debug(&emitter_for_llm, "llm: done");
                let _ = emitter_for_llm.emit(
                    "voice-transcript",
                    &serde_json::json!({"role": "assistant", "text": full_response}).to_string(),
                );
            }
            Ok(full_response)
        })
    });

    // Main thread: synthesize and play each sentence as it arrives
    ctx.audio_in.mute();
    let tts_sample_rate = ctx.tts_engine.sample_rate();
    let playback = ctx.audio_out.start_streaming(tts_sample_rate)?;
    let mut started_speaking = false;

    for sentence in sentence_rx {
        if ctx.stop.load(Ordering::Relaxed) {
            break;
        }
        if !started_speaking {
            emit_state(ctx.emitter, VoiceState::Speaking);
            started_speaking = true;
        }
        emit_debug(ctx.emitter, &format!("tts: \"{}\"", sentence));
        match ctx.tts_engine.synthesize(&sentence) {
            Ok(samples) => playback.push_samples(&samples),
            Err(e) => eprintln!("[voice] TTS error: {e}"),
        }
    }

    playback.finish();
    ctx.audio_in.unmute();
    emit_debug(ctx.emitter, "tts: done");
    std::thread::sleep(std::time::Duration::from_millis(300));
    ctx.vad_engine.reset();

    // Collect full response from LLM thread
    let response = match llm_handle.join() {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => return Err(e),
        Err(_) => return Err("LLM thread panicked".to_string()),
    };

    if !response.is_empty() {
        ctx.conversation.add_assistant(&response);
    }

    Ok(())
}
