//! E2E voice pipeline test using a pre-recorded speech WAV.
//! Feeds audio through: WAV → VAD → STT → LLM → TTS
//!
//! Run with: cargo test --test voice_e2e -- --nocapture
//! Requires:
//! - THE_CONTROLLER_RUN_VOICE_E2E=1
//! - models downloaded (~/.the-controller/voice-models/)
//! - codex CLI installed and authenticated

use std::path::Path;

fn read_wav_f32(path: &Path) -> Vec<f32> {
    let mut reader = hound::WavReader::open(path).expect("Failed to open WAV");
    let spec = reader.spec();
    assert_eq!(spec.channels, 1, "WAV must be mono");
    assert_eq!(spec.sample_rate, 16000, "WAV must be 16kHz");

    match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_val = (1 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.unwrap() as f32 / max_val)
                .collect()
        }
        hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap()).collect(),
    }
}

#[test]
#[ignore = "requires THE_CONTROLLER_RUN_VOICE_E2E=1, speech_sample.wav, and downloaded voice models"]
fn test_voice_pipeline_e2e() {
    let wav_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-data/speech_sample.wav");
    assert!(wav_path.exists(), "speech_sample.wav not found");

    let paths = the_controller_lib::voice::models::ModelPaths::new();
    assert!(paths.all_present(), "voice models not downloaded");

    // 1. Read WAV
    let audio = read_wav_f32(&wav_path);
    assert!(audio.len() > 16000, "WAV too short (need >1s)");
    println!(
        "WAV: {} samples ({:.1}s)",
        audio.len(),
        audio.len() as f32 / 16000.0
    );

    // 2. Feed through VAD
    let mut vad = the_controller_lib::voice::vad::Vad::new(&paths.silero_vad, 800)
        .expect("Failed to load VAD");

    let silence = vec![0.0f32; 8000];
    let padded: Vec<f32> = silence
        .iter()
        .chain(audio.iter())
        .chain(silence.iter())
        .copied()
        .collect();

    let mut speech_segments: Vec<Vec<f32>> = Vec::new();
    let mut current_segment: Vec<f32> = Vec::new();
    let mut in_speech = false;

    for chunk in padded.chunks(512) {
        if chunk.len() < 512 {
            break;
        }
        match vad.process(chunk).expect("VAD failed") {
            Some(the_controller_lib::voice::vad::VadEvent::SpeechStart) => {
                in_speech = true;
                current_segment.clear();
                current_segment.extend_from_slice(chunk);
            }
            Some(the_controller_lib::voice::vad::VadEvent::SpeechEnd) => {
                if in_speech {
                    current_segment.extend_from_slice(chunk);
                    speech_segments.push(current_segment.clone());
                    in_speech = false;
                }
            }
            None => {
                if in_speech {
                    current_segment.extend_from_slice(chunk);
                }
            }
        }
    }

    println!("VAD: {} speech segments detected", speech_segments.len());
    assert!(!speech_segments.is_empty(), "VAD detected no speech in WAV");

    // 3. STT
    let whisper = the_controller_lib::voice::stt::WhisperStt::new(&paths.whisper)
        .expect("Failed to load Whisper");

    let speech_audio = &speech_segments[0];
    let text = whisper.transcribe(speech_audio).expect("STT failed");
    println!("STT: \"{}\"", text);
    assert!(!text.is_empty(), "STT produced empty transcription");

    // 4. LLM
    let mut app_server = the_controller_lib::voice::llm::CodexAppServer::start(None)
        .expect("Failed to start codex app-server");

    let mut response = String::new();
    let result = app_server.stream_response(&text, &mut |token| {
        response.push_str(token);
    });
    result.expect("LLM failed");

    println!("LLM: \"{}\"", response);
    assert!(!response.is_empty(), "LLM produced empty response");

    // 5. TTS
    let mut tts =
        the_controller_lib::voice::tts::PiperTts::new(&paths.piper_onnx, &paths.piper_config)
            .expect("Failed to load TTS");

    let tts_audio = tts.synthesize(&response).expect("TTS failed");
    println!(
        "TTS: {} samples ({:.1}s at {}Hz)",
        tts_audio.len(),
        tts_audio.len() as f32 / tts.sample_rate() as f32,
        tts.sample_rate()
    );
    assert!(!tts_audio.is_empty(), "TTS produced no audio");

    println!("\n=== E2E PASS: WAV → VAD → STT → LLM → TTS ===");
}
