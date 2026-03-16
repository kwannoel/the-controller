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
    Initializing,
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
const MIN_BARGEIN_SAMPLES: usize = 4800; // 300ms at 16kHz — sustained speech required to confirm barge-in

pub struct VoicePipeline {
    stop_flag: Arc<AtomicBool>,
    audio_thread: Option<std::thread::JoinHandle<()>>,
}

struct SpeechContext<'a> {
    whisper: &'a stt::WhisperStt,
    tts_engine: &'a mut tts::PiperTts,
    audio_out: &'a audio_output::AudioOutput,
    vad_engine: &'a mut vad::Vad,
    auto_gain: &'a mut gain::AutoGain,
    audio_rx: &'a Receiver<Vec<i16>>,
    emitter: &'a Arc<dyn EventEmitter>,
    stop: &'a Arc<AtomicBool>,
}

/// Result of processing speech — either completed normally or was interrupted.
enum SpeechResult {
    /// Completed normally.
    Done,
    /// User interrupted (barge-in). Contains the speech buffer to process next.
    Interrupted(Vec<f32>),
}

impl VoicePipeline {
    /// Start the voice pipeline. Downloads models if needed, then begins listening.
    ///
    /// Waits for pipeline initialization to complete before returning, so any
    /// init errors (model loading, codex app-server, mic capture) are propagated
    /// to the caller instead of being silently lost on the background thread.
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

        // Signal the frontend that heavy init is starting (model loading, codex handshake)
        emit_state(&emitter, VoiceState::Initializing);

        let stop = stop_flag.clone();
        let emitter_clone = emitter.clone();

        let vad_path = model_paths.silero_vad.clone();
        let whisper_path = model_paths.whisper.clone();
        let piper_onnx_path = model_paths.piper_onnx.clone();
        let piper_config_path = model_paths.piper_config.clone();

        let (init_tx, init_rx) = tokio::sync::oneshot::channel::<Result<(), String>>();

        let audio_thread = std::thread::spawn(move || {
            let init_result = pipeline_init(
                &vad_path,
                &whisper_path,
                &piper_onnx_path,
                &piper_config_path,
            );

            match init_result {
                Err(e) => {
                    eprintln!("[voice] Pipeline init error: {e}");
                    let _ = init_tx.send(Err(e.clone()));
                    let payload = serde_json::json!({
                        "state": "error",
                        "error": e,
                    })
                    .to_string();
                    let _ = emitter_clone.emit("voice-state-changed", &payload);
                }
                Ok(components) => {
                    let _ = init_tx.send(Ok(()));
                    if let Err(e) =
                        run_pipeline_loop(components, stop, emitter_clone.clone())
                    {
                        eprintln!("[voice] Pipeline error: {e}");
                        let payload = serde_json::json!({
                            "state": "error",
                            "error": e,
                        })
                        .to_string();
                        let _ = emitter_clone.emit("voice-state-changed", &payload);
                    }
                }
            }
        });

        // Wait for pipeline initialization with timeout — propagate errors to caller
        match tokio::time::timeout(
            std::time::Duration::from_secs(30),
            init_rx,
        )
        .await
        {
            Ok(Ok(Ok(()))) => Ok(Self {
                stop_flag,
                audio_thread: Some(audio_thread),
            }),
            Ok(Ok(Err(e))) => Err(e),
            Ok(Err(_)) => Err("Pipeline thread panicked during initialization".to_string()),
            Err(_) => {
                // Timeout — signal thread to stop and return error
                stop_flag.store(true, Ordering::Relaxed);
                Err("Pipeline initialization timed out after 30s".to_string())
            }
        }
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

/// Holds all initialized pipeline components.
struct PipelineComponents {
    vad_engine: vad::Vad,
    whisper: stt::WhisperStt,
    tts_engine: tts::PiperTts,
    audio_out: audio_output::AudioOutput,
    auto_gain: gain::AutoGain,
    app_server: llm::CodexAppServer,
    audio_in: audio_input::AudioInput,
    audio_rx: Receiver<Vec<i16>>,
}

/// Initialize all pipeline components. Called on the pipeline thread.
fn pipeline_init(
    vad_path: &std::path::Path,
    whisper_path: &std::path::Path,
    piper_onnx_path: &std::path::Path,
    piper_config_path: &std::path::Path,
) -> Result<PipelineComponents, String> {
    let vad_engine = vad::Vad::new(vad_path, 800)?;
    let whisper = stt::WhisperStt::new(whisper_path)?;
    let tts_engine = tts::PiperTts::new(piper_onnx_path, piper_config_path)?;
    let audio_out = audio_output::AudioOutput::new()?;
    let auto_gain = gain::AutoGain::new();
    let app_server = llm::CodexAppServer::start(None)
        .map_err(|e| format!("Failed to start codex app-server: {e}"))?;
    let (tx, audio_rx): (Sender<Vec<i16>>, Receiver<Vec<i16>>) = crossbeam_channel::bounded(64);
    let audio_in = audio_input::AudioInput::start(tx)?;

    Ok(PipelineComponents {
        vad_engine,
        whisper,
        tts_engine,
        audio_out,
        auto_gain,
        app_server,
        audio_in,
        audio_rx,
    })
}

/// Main pipeline loop. Runs on a dedicated thread after initialization.
fn run_pipeline_loop(
    components: PipelineComponents,
    stop: Arc<AtomicBool>,
    emitter: Arc<dyn EventEmitter>,
) -> Result<(), String> {
    let PipelineComponents {
        mut vad_engine,
        whisper,
        mut tts_engine,
        audio_out,
        mut auto_gain,
        mut app_server,
        mut audio_in,
        audio_rx: rx,
    } = components;

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
                        vad_engine: &mut vad_engine,
                        auto_gain: &mut auto_gain,
                        audio_rx: &rx,
                        emitter: &emitter,
                        stop: &stop,
                    };

                    // Pass app_server by value; get it back after the turn completes.
                    let (result, returned_server) =
                        process_speech(&speech_buffer, &mut speech_ctx, app_server)?;
                    app_server = returned_server;

                    speech_buffer.clear();

                    // If interrupted, seed the speech buffer with the barge-in audio
                    // and resume the normal VAD listening loop to collect the full utterance
                    if let SpeechResult::Interrupted(barge_in_audio) = result {
                        speech_buffer.extend_from_slice(&barge_in_audio);
                        in_speech = true;
                    }

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

/// Process a speech segment: transcribe, stream LLM response, synthesize TTS.
///
/// Takes `app_server` by value (the LLM thread needs ownership during the turn)
/// and returns it alongside the result. This eliminates all unsafe raw pointers.
fn process_speech(
    audio: &[f32],
    ctx: &mut SpeechContext<'_>,
    app_server: llm::CodexAppServer,
) -> Result<(SpeechResult, llm::CodexAppServer), String> {
    if audio.len() < MIN_SPEECH_SAMPLES {
        return Ok((SpeechResult::Done, app_server));
    }

    let rms = (audio.iter().map(|&s| s * s).sum::<f32>() / audio.len() as f32).sqrt();
    if rms < MIN_SPEECH_RMS {
        return Ok((SpeechResult::Done, app_server));
    }

    // STT
    emit_state(ctx.emitter, VoiceState::Thinking);
    let text = ctx.whisper.transcribe(audio)?;
    if text.is_empty() {
        return Ok((SpeechResult::Done, app_server));
    }

    eprintln!("[voice] You: {text}");
    emit_debug(ctx.emitter, &format!("stt: \"{}\"", text));
    let _ = ctx.emitter.emit(
        "voice-transcript",
        &serde_json::json!({"role": "user", "text": text}).to_string(),
    );

    // Stream LLM -> TTS -> Audio concurrently.
    // LLM runs in a background thread (owning app_server), splitting tokens into sentences.
    // Main thread synthesizes each sentence via TTS and streams audio immediately.
    emit_debug(ctx.emitter, "llm: streaming...");

    let emitter_for_llm = ctx.emitter.clone();
    let (sentence_tx, sentence_rx) = crossbeam_channel::bounded::<String>(8);
    let user_text = text.clone();

    // The LLM thread takes ownership of app_server and returns it when done.
    // No unsafe code, no raw pointers, no data races.
    let llm_handle = std::thread::spawn(
        move || -> (llm::CodexAppServer, Result<String, String>) {
            let mut app_server = app_server;
            let mut sentence_buf = String::new();
            let mut full_response = String::new();

            let result = app_server.stream_response(&user_text, &mut |token| {
                sentence_buf.push_str(token);
                full_response.push_str(token);
                while let Some(pos) =
                    sentence_buf.find(['.', '!', '?'])
                {
                    let sentence = sentence_buf[..=pos].trim().to_string();
                    sentence_buf = sentence_buf[pos + 1..].to_string();
                    if !sentence.is_empty() {
                        let _ = sentence_tx.send(sentence);
                    }
                }
            });

            match result {
                Ok(_) => {
                    let remaining = sentence_buf.trim().to_string();
                    if !remaining.is_empty() {
                        let _ = sentence_tx.send(remaining);
                    }
                    if !full_response.is_empty() {
                        eprintln!("[voice] Assistant: {full_response}");
                        emit_debug(&emitter_for_llm, "llm: done");
                        let _ = emitter_for_llm.emit(
                            "voice-transcript",
                            &serde_json::json!({"role": "assistant", "text": full_response})
                                .to_string(),
                        );
                    }
                    (app_server, Ok(full_response))
                }
                Err(e) => (app_server, Err(e)),
            }
        },
    );

    // Synthesize and play sentences while monitoring mic for barge-in.
    // Mic stays open — requires headphones to avoid echo feedback.
    let tts_sample_rate = ctx.tts_engine.sample_rate();
    let playback = ctx.audio_out.start_streaming(tts_sample_rate)?;
    let mut spoken_sentences: Vec<String> = Vec::new();
    let mut started_speaking = false;
    let mut barge_in_speech: Option<Vec<f32>> = None;

    loop {
        crossbeam_channel::select! {
            recv(sentence_rx) -> msg => {
                match msg {
                    Ok(sentence) => {
                        if ctx.stop.load(Ordering::Relaxed) {
                            break;
                        }
                        if !started_speaking {
                            emit_state(ctx.emitter, VoiceState::Speaking);
                            started_speaking = true;
                        }
                        let clean = strip_markdown(&sentence);
                        if clean.is_empty() {
                            continue;
                        }
                        spoken_sentences.push(clean.clone());
                        emit_debug(ctx.emitter, &format!("tts: \"{}\"", clean));
                        match ctx.tts_engine.synthesize(&clean) {
                            Ok(samples) => playback.push_samples(&samples),
                            Err(e) => eprintln!("[voice] TTS error: {e}"),
                        }
                    }
                    Err(_) => break, // LLM done, channel closed
                }
            }
            recv(ctx.audio_rx) -> msg => {
                if let Ok(chunk) = msg {
                    if !started_speaking {
                        continue;
                    }
                    let normalized = ctx.auto_gain.apply(&chunk);
                    if let Some(vad::VadEvent::SpeechStart) = ctx.vad_engine.process(&normalized)? {
                        // Require sustained speech to confirm barge-in (not just a noise spike)
                        let mut speech_buf = Vec::new();
                        speech_buf.extend_from_slice(&normalized);
                        let mut confirmed = true;
                        while speech_buf.len() < MIN_BARGEIN_SAMPLES {
                            match ctx.audio_rx.recv_timeout(std::time::Duration::from_millis(50)) {
                                Ok(more) => {
                                    let norm = ctx.auto_gain.apply(&more);
                                    if let Some(vad::VadEvent::SpeechEnd) = ctx.vad_engine.process(&norm)? {
                                        confirmed = false; // Noise spike, not real speech
                                        break;
                                    }
                                    speech_buf.extend_from_slice(&norm);
                                }
                                Err(_) => { confirmed = false; break; }
                            }
                        }
                        if confirmed {
                            emit_debug(ctx.emitter, "barge-in: confirmed, cancelling");
                            while let Ok(more) = ctx.audio_rx.try_recv() {
                                let norm = ctx.auto_gain.apply(&more);
                                speech_buf.extend_from_slice(&norm);
                            }
                            barge_in_speech = Some(speech_buf);
                            break;
                        }
                        // False alarm — VAD reset via SpeechEnd, continue
                    }
                }
            }
        }
    }

    let interrupted = barge_in_speech.is_some();

    if interrupted {
        playback.cancel();
        // Do NOT call cancel_turn() here — the LLM thread owns app_server.
        // Instead, we join the LLM thread below to wait for the turn to finish.
        // This avoids the data race where both threads access app_server concurrently.
    } else {
        // Signal no more audio will be pushed so is_done() can return true
        playback.seal();
        // Wait for remaining audio to finish playing
        if !playback.is_done() {
            // Keep monitoring mic while audio drains
            while !playback.is_done() {
                if let Ok(chunk) = ctx.audio_rx.recv_timeout(std::time::Duration::from_millis(10)) {
                    let normalized = ctx.auto_gain.apply(&chunk);
                    if let Some(vad::VadEvent::SpeechStart) =
                        ctx.vad_engine.process(&normalized)?
                    {
                        // Confirm sustained speech before triggering barge-in
                        let mut speech_buf = Vec::new();
                        speech_buf.extend_from_slice(&normalized);
                        let mut confirmed = true;
                        while speech_buf.len() < MIN_BARGEIN_SAMPLES {
                            match ctx.audio_rx.recv_timeout(std::time::Duration::from_millis(50)) {
                                Ok(more) => {
                                    let norm = ctx.auto_gain.apply(&more);
                                    if let Some(vad::VadEvent::SpeechEnd) = ctx.vad_engine.process(&norm)? {
                                        confirmed = false;
                                        break;
                                    }
                                    speech_buf.extend_from_slice(&norm);
                                }
                                Err(_) => { confirmed = false; break; }
                            }
                        }
                        if confirmed {
                            emit_debug(ctx.emitter, "barge-in: confirmed during drain");
                            while let Ok(more) = ctx.audio_rx.try_recv() {
                                let norm = ctx.auto_gain.apply(&more);
                                speech_buf.extend_from_slice(&norm);
                            }
                            barge_in_speech = Some(speech_buf);
                            playback.cancel();
                            break;
                        }
                    }
                } // timeout: check is_done again
            }
            if barge_in_speech.is_none() {
                // Finished naturally
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }

    emit_debug(ctx.emitter, if barge_in_speech.is_some() { "tts: cancelled (barge-in)" } else { "tts: done" });

    if barge_in_speech.is_none() {
        // Normal completion — pause for audio settle and reset VAD
        std::thread::sleep(std::time::Duration::from_millis(300));
        ctx.vad_engine.reset();
    }
    // On barge-in: don't reset VAD — it's in triggered state tracking the ongoing speech

    // Always join the LLM thread to reclaim ownership of app_server.
    // On barge-in, this waits for the turn to finish server-side (safe, no data race).
    // On normal completion, the thread is already done or nearly done.
    let (app_server, llm_result) = llm_handle
        .join()
        .map_err(|_| "LLM thread panicked".to_string())?;

    if interrupted {
        let partial = spoken_sentences.join(" ");
        if !partial.is_empty() {
            eprintln!("[voice] Assistant (interrupted): {partial}");
            let _ = ctx.emitter.emit(
                "voice-transcript",
                &serde_json::json!({"role": "assistant", "text": format!("{partial}…")}).to_string(),
            );
        }
    } else {
        // Check the LLM result for errors
        llm_result?;
    }

    let speech_result = if let Some(speech) = barge_in_speech {
        SpeechResult::Interrupted(speech)
    } else {
        SpeechResult::Done
    };

    Ok((speech_result, app_server))
}

/// Strip markdown formatting so TTS doesn't read formatting characters aloud.
fn strip_markdown(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for line in text.lines() {
        let line = line.trim();
        // Strip heading markers
        let line = line.trim_start_matches('#').trim_start();
        // Strip blockquote markers
        let line = line.trim_start_matches('>').trim_start();
        // Strip list markers ("- ", "* ", "1. ", "2. ", etc.)
        let line = if line.starts_with("- ") || line.starts_with("* ") {
            &line[2..]
        } else if line.len() >= 3
            && line.as_bytes()[0].is_ascii_digit()
            && line.as_bytes().get(1) == Some(&b'.')
            && line.as_bytes().get(2) == Some(&b' ')
        {
            &line[3..]
        } else {
            line
        };
        if !result.is_empty() && !line.is_empty() {
            result.push(' ');
        }
        result.push_str(line);
    }
    // Remove inline formatting: *, `, ~
    result.retain(|c| !matches!(c, '*' | '`' | '~'));
    // Collapse multiple spaces
    let mut prev_space = false;
    result = result
        .chars()
        .filter(|&c| {
            if c == ' ' {
                if prev_space {
                    return false;
                }
                prev_space = true;
            } else {
                prev_space = false;
            }
            true
        })
        .collect();
    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::strip_markdown;

    #[test]
    fn strips_bold_and_italic() {
        assert_eq!(strip_markdown("**hello** world"), "hello world");
        assert_eq!(strip_markdown("*italic* text"), "italic text");
        assert_eq!(strip_markdown("***both***"), "both");
    }

    #[test]
    fn strips_backticks() {
        assert_eq!(strip_markdown("use `println!` here"), "use println! here");
        assert_eq!(strip_markdown("```code block```"), "code block");
    }

    #[test]
    fn strips_headings() {
        assert_eq!(strip_markdown("## My Heading"), "My Heading");
        assert_eq!(strip_markdown("# Title"), "Title");
    }

    #[test]
    fn strips_list_markers() {
        assert_eq!(strip_markdown("- item one\n- item two"), "item one item two");
        assert_eq!(strip_markdown("* bullet"), "bullet");
        assert_eq!(strip_markdown("1. first\n2. second"), "first second");
    }

    #[test]
    fn strips_blockquotes() {
        assert_eq!(strip_markdown("> quoted text"), "quoted text");
    }

    #[test]
    fn strips_strikethrough() {
        assert_eq!(strip_markdown("~~removed~~ kept"), "removed kept");
    }

    #[test]
    fn preserves_plain_text() {
        assert_eq!(strip_markdown("Hello, how are you?"), "Hello, how are you?");
    }

    #[test]
    fn handles_empty_input() {
        assert_eq!(strip_markdown(""), "");
        assert_eq!(strip_markdown("***"), "");
    }
}
