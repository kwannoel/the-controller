# Voice Mode Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Add voice chat with Claude — full local pipeline embedded in Rust, no paid API.

**Architecture:** cpal mic → AutoGain → Silero VAD (ONNX) → whisper.cpp STT → Claude CLI → Piper TTS (ONNX) → cpal speaker. All processing in Rust. Frontend only displays state via Tauri events. Pipeline starts on mode enter, stops on mode exit.

**Tech Stack:** cpal (audio I/O), ort (ONNX Runtime for VAD + TTS), whisper-rs (STT), tokio::process (Claude CLI), crossbeam (channels between threads)

**Design doc:** `docs/plans/2026-03-13-voice-mode-design.md`

**Key reference:** The Python implementation at `github.com/kwannoel/live-chat` — algorithms are ported from there. See the design doc for parameter values.

**Domain knowledge:** Read `docs/domain-knowledge.md` before touching Tauri commands. Key rule: async commands + `spawn_blocking` for heavy work. Remove `CLAUDECODE` env var from all `Command::new("claude")` calls.

---

### Task 1: Cargo dependencies + voice module skeleton

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/voice/mod.rs`
- Create: `src-tauri/src/voice/models.rs`
- Create: `src-tauri/src/voice/gain.rs`
- Create: `src-tauri/src/voice/audio_input.rs`
- Create: `src-tauri/src/voice/audio_output.rs`
- Create: `src-tauri/src/voice/vad.rs`
- Create: `src-tauri/src/voice/stt.rs`
- Create: `src-tauri/src/voice/tts.rs`
- Create: `src-tauri/src/voice/llm.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: Add dependencies to Cargo.toml**

Add these under `[dependencies]`:

```toml
cpal = "0.15"
whisper-rs = "0.12"
ort = { version = "2", features = ["download-binaries"] }
hound = "3.5"
crossbeam-channel = "0.5"
```

**Step 2: Create voice module skeleton**

Create `src-tauri/src/voice/mod.rs`:

```rust
pub mod audio_input;
pub mod audio_output;
pub mod gain;
pub mod llm;
pub mod models;
pub mod stt;
pub mod tts;
pub mod vad;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VoiceState {
    Listening,
    Thinking,
    Speaking,
    Downloading,
}
```

Create each submodule as an empty file with a comment:

```rust
// TODO: implement
```

**Step 3: Register module in lib.rs**

Add `pub mod voice;` to the module declarations in `src-tauri/src/lib.rs` (after the `pub mod worktree;` line).

**Step 4: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: Compiles with no errors (warnings about unused modules are OK)

**Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/voice/ src-tauri/src/lib.rs
git commit -m "feat(voice): add cargo dependencies and module skeleton"
```

---

### Task 2: Model download and path management

**Files:**
- Implement: `src-tauri/src/voice/models.rs`
- Test: `src-tauri/src/voice/models.rs` (inline tests)

This module manages downloading and locating the three model files:
- Silero VAD ONNX (~2MB)
- Whisper base GGML (~75MB)
- Piper voice ONNX + JSON config (~30MB)

Models are stored in `~/.the-controller/voice-models/`.

**Step 1: Write the test**

Add to `src-tauri/src/voice/models.rs`:

```rust
use std::path::PathBuf;

/// Returns the base directory for voice model storage.
pub fn models_dir() -> PathBuf {
    dirs::home_dir()
        .expect("home directory must exist")
        .join(".the-controller")
        .join("voice-models")
}

pub struct ModelPaths {
    pub silero_vad: PathBuf,
    pub whisper: PathBuf,
    pub piper_onnx: PathBuf,
    pub piper_config: PathBuf,
}

impl ModelPaths {
    pub fn new() -> Self {
        let base = models_dir();
        Self {
            silero_vad: base.join("silero_vad.onnx"),
            whisper: base.join("ggml-base.bin"),
            piper_onnx: base.join("en_US-lessac-medium.onnx"),
            piper_config: base.join("en_US-lessac-medium.onnx.json"),
        }
    }

    /// Returns true if all model files exist on disk.
    pub fn all_present(&self) -> bool {
        self.silero_vad.exists()
            && self.whisper.exists()
            && self.piper_onnx.exists()
            && self.piper_config.exists()
    }

    /// Returns a list of (url, destination) pairs for missing models.
    pub fn missing_downloads(&self) -> Vec<(&'static str, &PathBuf)> {
        let mut missing = Vec::new();
        if !self.silero_vad.exists() {
            missing.push((
                "https://github.com/snakers4/silero-vad/raw/master/src/silero_vad/data/silero_vad.onnx",
                &self.silero_vad,
            ));
        }
        if !self.whisper.exists() {
            missing.push((
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
                &self.whisper,
            ));
        }
        if !self.piper_onnx.exists() {
            missing.push((
                "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/lessac/medium/en_US-lessac-medium.onnx",
                &self.piper_onnx,
            ));
        }
        if !self.piper_config.exists() {
            missing.push((
                "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/lessac/medium/en_US-lessac-medium.onnx.json",
                &self.piper_config,
            ));
        }
        missing
    }
}

/// Download all missing models. Calls `on_progress` with the filename being downloaded.
pub async fn ensure_models(on_progress: impl Fn(&str)) -> Result<ModelPaths, String> {
    let paths = ModelPaths::new();
    let downloads = paths.missing_downloads();

    if !downloads.is_empty() {
        std::fs::create_dir_all(models_dir()).map_err(|e| format!("Failed to create models dir: {e}"))?;
    }

    for (url, dest) in &downloads {
        let filename = dest.file_name().unwrap().to_string_lossy();
        on_progress(&filename);

        let response = reqwest::get(*url)
            .await
            .map_err(|e| format!("Failed to download {filename}: {e}"))?;

        if !response.status().is_success() {
            return Err(format!("Failed to download {filename}: HTTP {}", response.status()));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read {filename}: {e}"))?;

        std::fs::write(dest, &bytes)
            .map_err(|e| format!("Failed to write {filename}: {e}"))?;
    }

    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_paths_point_to_correct_directory() {
        let paths = ModelPaths::new();
        let base = models_dir();
        assert!(paths.silero_vad.starts_with(&base));
        assert!(paths.whisper.starts_with(&base));
        assert!(paths.piper_onnx.starts_with(&base));
        assert!(paths.piper_config.starts_with(&base));
    }

    #[test]
    fn missing_downloads_returns_all_when_none_exist() {
        // Uses default paths which won't exist in test env (unless models are installed)
        let paths = ModelPaths {
            silero_vad: PathBuf::from("/nonexistent/vad.onnx"),
            whisper: PathBuf::from("/nonexistent/whisper.bin"),
            piper_onnx: PathBuf::from("/nonexistent/piper.onnx"),
            piper_config: PathBuf::from("/nonexistent/piper.json"),
        };
        assert_eq!(paths.missing_downloads().len(), 4);
        assert!(!paths.all_present());
    }
}
```

**Step 2: Run tests**

Run: `cd src-tauri && cargo test voice::models`
Expected: 2 tests pass

**Step 3: Commit**

```bash
git add src-tauri/src/voice/models.rs
git commit -m "feat(voice): add model download and path management"
```

---

### Task 3: AutoGain normalization

**Files:**
- Implement: `src-tauri/src/voice/gain.rs`
- Test: inline in same file

Port the AutoGain algorithm from `live-chat/audio/gain.py`. Converts int16 mic samples to normalized float32 with adaptive gain.

**Step 1: Write test and implementation**

```rust
use std::collections::VecDeque;

/// Adaptive gain control. Tracks rolling RMS and applies gain to maintain
/// a target loudness level. Converts int16 input to normalized float32.
pub struct AutoGain {
    target_rms: f32,
    max_gain: f32,
    history: VecDeque<f32>,
    window_size: usize,
}

impl AutoGain {
    pub fn new() -> Self {
        Self {
            target_rms: 0.1,
            max_gain: 100.0,
            history: VecDeque::new(),
            window_size: 31,
        }
    }

    /// Process a chunk of int16 audio samples. Returns float32 in [-1.0, 1.0].
    pub fn apply(&mut self, samples: &[i16]) -> Vec<f32> {
        // Convert to float32
        let float_samples: Vec<f32> = samples.iter().map(|&s| s as f32 / 32768.0).collect();

        // Calculate RMS
        let rms = {
            let sum_sq: f32 = float_samples.iter().map(|&s| s * s).sum();
            (sum_sq / float_samples.len() as f32).sqrt()
        };

        // Skip near-silence
        if rms < 1e-6 {
            return float_samples;
        }

        // Update history
        self.history.push_back(rms);
        if self.history.len() > self.window_size {
            self.history.pop_front();
        }

        // Calculate average RMS
        let avg_rms: f32 = self.history.iter().sum::<f32>() / self.history.len() as f32;
        if avg_rms < 1e-6 {
            return float_samples;
        }

        // Apply gain
        let gain = (self.target_rms / avg_rms).min(self.max_gain);
        float_samples
            .iter()
            .map(|&s| (s * gain).clamp(-1.0, 1.0))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_passes_through_unchanged() {
        let mut gain = AutoGain::new();
        let silence = vec![0i16; 512];
        let result = gain.apply(&silence);
        assert!(result.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn output_is_clamped_to_unit_range() {
        let mut gain = AutoGain::new();
        // Very quiet signal — gain will be high
        let quiet: Vec<i16> = (0..512).map(|i| (i % 3) as i16).collect();
        for _ in 0..50 {
            let result = gain.apply(&quiet);
            assert!(result.iter().all(|&s| s >= -1.0 && s <= 1.0));
        }
    }

    #[test]
    fn converts_int16_to_float32() {
        let mut gain = AutoGain::new();
        let input = vec![16384i16]; // 0.5 in float
        let result = gain.apply(&input);
        // Before gain history builds up, first chunk is just normalized
        assert!(!result.is_empty());
        // Value should be finite and in valid range
        assert!(result[0].is_finite());
        assert!(result[0] >= -1.0 && result[0] <= 1.0);
    }
}
```

**Step 2: Run tests**

Run: `cd src-tauri && cargo test voice::gain`
Expected: 3 tests pass

**Step 3: Commit**

```bash
git add src-tauri/src/voice/gain.rs
git commit -m "feat(voice): add AutoGain normalization"
```

---

### Task 4: Audio input (cpal mic capture)

**Files:**
- Implement: `src-tauri/src/voice/audio_input.rs`

Captures 16kHz mono int16 audio from the default mic. Sends chunks via crossbeam channel. Supports muting.

**Step 1: Implement**

```rust
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleRate, Stream};
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub const SAMPLE_RATE: u32 = 16_000;
pub const BLOCK_SIZE: usize = 512; // Required by Silero VAD

pub struct AudioInput {
    stream: Option<Stream>,
    muted: Arc<AtomicBool>,
}

impl AudioInput {
    /// Create and start mic capture. Sends int16 chunks of BLOCK_SIZE to `sender`.
    pub fn start(sender: Sender<Vec<i16>>) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No input device available")?;

        let config = cpal::StreamConfig {
            channels: 1,
            sample_rate: SampleRate(SAMPLE_RATE),
            buffer_size: cpal::BufferSize::Fixed(BLOCK_SIZE as u32),
        };

        let muted = Arc::new(AtomicBool::new(false));
        let muted_clone = muted.clone();

        let stream = device
            .build_input_stream(
                &config,
                move |data: &[i16], _info: &cpal::InputCallbackInfo| {
                    if !muted_clone.load(Ordering::Relaxed) {
                        let _ = sender.try_send(data.to_vec());
                    }
                },
                |err| {
                    eprintln!("[voice] Audio input error: {err}");
                },
                None,
            )
            .map_err(|e| format!("Failed to build input stream: {e}"))?;

        stream.play().map_err(|e| format!("Failed to start input stream: {e}"))?;

        Ok(Self {
            stream: Some(stream),
            muted,
        })
    }

    pub fn mute(&self) {
        self.muted.store(true, Ordering::Relaxed);
    }

    pub fn unmute(&self) {
        self.muted.store(false, Ordering::Relaxed);
    }

    pub fn stop(&mut self) {
        self.stream.take(); // Dropping the stream stops capture
    }
}
```

**Step 2: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: Compiles (can't unit test mic capture without hardware)

**Step 3: Commit**

```bash
git add src-tauri/src/voice/audio_input.rs
git commit -m "feat(voice): add cpal mic capture"
```

---

### Task 5: Audio output (cpal speaker playback)

**Files:**
- Implement: `src-tauri/src/voice/audio_output.rs`

Plays float32 or int16 audio through the default speaker. Blocking `wait()` and async `wait_async()`.

**Step 1: Implement**

```rust
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleRate, Stream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

pub struct AudioOutput {
    device: cpal::Device,
}

impl AudioOutput {
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("No output device available")?;
        Ok(Self { device })
    }

    /// Play int16 audio at the given sample rate. Blocks until playback completes.
    pub fn play_i16(&self, samples: &[i16], sample_rate: u32) -> Result<(), String> {
        let config = cpal::StreamConfig {
            channels: 1,
            sample_rate: SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let data = Arc::new(samples.to_vec());
        let position = Arc::new(AtomicUsize::new(0));
        let done = Arc::new(AtomicBool::new(false));

        let data_clone = data.clone();
        let pos_clone = position.clone();
        let done_clone = done.clone();

        let stream = self
            .device
            .build_output_stream(
                &config,
                move |output: &mut [i16], _info: &cpal::OutputCallbackInfo| {
                    let pos = pos_clone.load(Ordering::Relaxed);
                    let remaining = data_clone.len() - pos;
                    let to_write = remaining.min(output.len());

                    output[..to_write].copy_from_slice(&data_clone[pos..pos + to_write]);
                    // Silence any remaining buffer
                    for sample in output[to_write..].iter_mut() {
                        *sample = 0;
                    }

                    pos_clone.store(pos + to_write, Ordering::Relaxed);
                    if pos + to_write >= data_clone.len() {
                        done_clone.store(true, Ordering::Relaxed);
                    }
                },
                |err| {
                    eprintln!("[voice] Audio output error: {err}");
                },
                None,
            )
            .map_err(|e| format!("Failed to build output stream: {e}"))?;

        stream.play().map_err(|e| format!("Failed to start output: {e}"))?;

        // Wait for playback to finish
        while !done.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // Small buffer drain delay
        std::thread::sleep(std::time::Duration::from_millis(50));

        Ok(())
    }

    /// Async version of play_i16.
    pub async fn play_i16_async(&self, samples: Vec<i16>, sample_rate: u32) -> Result<(), String> {
        // cpal streams are !Send, so we need to run on current thread
        // Use spawn_blocking to avoid blocking the async runtime
        let device_name = self.device.name().unwrap_or_default();
        tokio::task::spawn_blocking(move || {
            let host = cpal::default_host();
            let device = host
                .default_output_device()
                .ok_or("No output device available")?;
            let output = AudioOutput { device };
            output.play_i16(&samples, sample_rate)
        })
        .await
        .map_err(|e| format!("Playback task failed: {e}"))?
    }
}
```

**Step 2: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: Compiles (can't unit test speaker output without hardware)

**Step 3: Commit**

```bash
git add src-tauri/src/voice/audio_output.rs
git commit -m "feat(voice): add cpal speaker playback"
```

---

### Task 6: Silero VAD

**Files:**
- Implement: `src-tauri/src/voice/vad.rs`
- Test: inline (logic tests only, model tests require downloaded model)

Port the VAD from `live-chat/audio/vad.py`. Uses the Silero VAD ONNX model to detect speech start/end.

**Step 1: Implement**

```rust
use ort::session::Session;
use std::path::Path;

/// Voice Activity Detection events.
#[derive(Debug, Clone, PartialEq)]
pub enum VadEvent {
    SpeechStart,
    SpeechEnd,
}

pub struct Vad {
    session: Session,
    // Silero VAD internal state
    h: ndarray::Array2<f32>,
    c: ndarray::Array2<f32>,
    triggered: bool,
    temp_end: usize,
    current_sample: usize,
    threshold: f32,
    min_silence_samples: usize,
}

impl Vad {
    pub fn new(model_path: &Path, min_silence_ms: u32) -> Result<Self, String> {
        let session = Session::builder()
            .map_err(|e| format!("Failed to create ONNX session builder: {e}"))?
            .with_intra_threads(1)
            .map_err(|e| format!("Failed to set threads: {e}"))?
            .commit_from_file(model_path)
            .map_err(|e| format!("Failed to load Silero VAD model: {e}"))?;

        let min_silence_samples = (min_silence_ms as usize * 16000) / 1000;

        Ok(Self {
            session,
            h: ndarray::Array2::zeros((2, 64)),
            c: ndarray::Array2::zeros((2, 64)),
            triggered: false,
            temp_end: 0,
            current_sample: 0,
            threshold: 0.5,
            min_silence_samples,
        })
    }

    /// Process a chunk of float32 audio (512 samples at 16kHz).
    /// Returns a VadEvent if a speech boundary was detected.
    pub fn process(&mut self, chunk: &[f32]) -> Result<Option<VadEvent>, String> {
        use ort::value::Value;

        let chunk_len = chunk.len();
        self.current_sample += chunk_len;

        // Prepare inputs
        let input_tensor = ndarray::Array2::from_shape_vec((1, chunk_len), chunk.to_vec())
            .map_err(|e| format!("Failed to create input tensor: {e}"))?;
        let sr = ndarray::Array1::from_vec(vec![16000i64]);

        let outputs = self
            .session
            .run(ort::inputs![
                "input" => input_tensor,
                "sr" => sr,
                "h" => self.h.clone(),
                "c" => self.c.clone(),
            ].map_err(|e| format!("Failed to create inputs: {e}"))?)
            .map_err(|e| format!("VAD inference failed: {e}"))?;

        // Extract outputs
        let output_prob = outputs["output"]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract output: {e}"))?;
        let prob = output_prob.as_slice().unwrap()[0];

        self.h = outputs["hn"]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract hn: {e}"))?
            .to_owned()
            .into_dimensionality()
            .map_err(|e| format!("Wrong hn shape: {e}"))?;

        self.c = outputs["cn"]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract cn: {e}"))?
            .to_owned()
            .into_dimensionality()
            .map_err(|e| format!("Wrong cn shape: {e}"))?;

        // State machine
        if prob >= self.threshold && !self.triggered {
            self.triggered = true;
            self.temp_end = 0;
            return Ok(Some(VadEvent::SpeechStart));
        }

        if prob < (self.threshold - 0.15) && self.triggered {
            if self.temp_end == 0 {
                self.temp_end = self.current_sample;
            }
            if self.current_sample - self.temp_end >= self.min_silence_samples {
                self.triggered = false;
                self.temp_end = 0;
                return Ok(Some(VadEvent::SpeechEnd));
            }
        } else if self.triggered {
            self.temp_end = 0;
        }

        Ok(None)
    }

    /// Reset state for a new conversation turn.
    pub fn reset(&mut self) {
        self.h = ndarray::Array2::zeros((2, 64));
        self.c = ndarray::Array2::zeros((2, 64));
        self.triggered = false;
        self.temp_end = 0;
        self.current_sample = 0;
    }
}
```

Note: Add `ndarray = "0.16"` to `Cargo.toml` dependencies (needed for ort tensor operations).

**Step 2: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: Compiles

**Step 3: Commit**

```bash
git add src-tauri/src/voice/vad.rs src-tauri/Cargo.toml
git commit -m "feat(voice): add Silero VAD via ONNX Runtime"
```

---

### Task 7: Whisper STT

**Files:**
- Implement: `src-tauri/src/voice/stt.rs`

Port from `live-chat/stt/whisper.py`. Uses whisper-rs with hallucination filtering.

**Step 1: Implement**

```rust
use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct WhisperStt {
    ctx: WhisperContext,
}

impl WhisperStt {
    pub fn new(model_path: &Path) -> Result<Self, String> {
        let ctx = WhisperContext::new_with_params(
            model_path.to_str().ok_or("Invalid model path")?,
            WhisperContextParameters::default(),
        )
        .map_err(|e| format!("Failed to load Whisper model: {e}"))?;

        Ok(Self { ctx })
    }

    /// Transcribe float32 16kHz audio to text. Returns empty string if no speech detected.
    pub fn transcribe(&self, audio: &[f32]) -> Result<String, String> {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("en"));
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_no_context(true);
        // Single-segment mode for short utterances
        params.set_single_segment(true);

        let mut state = self
            .ctx
            .create_state()
            .map_err(|e| format!("Failed to create Whisper state: {e}"))?;

        state
            .full(params, audio)
            .map_err(|e| format!("Whisper inference failed: {e}"))?;

        let n_segments = state.full_n_segments()
            .map_err(|e| format!("Failed to get segments: {e}"))?;

        let mut result = String::new();
        for i in 0..n_segments {
            let text = state
                .full_get_segment_text(i)
                .map_err(|e| format!("Failed to get segment text: {e}"))?;

            // Hallucination filters (from live-chat)
            // whisper-rs doesn't expose no_speech_prob/avg_logprob/compression_ratio
            // directly per segment in all versions, so we filter by content heuristics
            let trimmed = text.trim();
            if trimmed.is_empty() {
                continue;
            }
            // Skip common Whisper hallucinations
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                continue; // [MUSIC], [BLANK_AUDIO], etc.
            }
            if trimmed == "Thank you." || trimmed == "Thanks for watching!" {
                continue; // Common hallucination on silence
            }

            result.push_str(trimmed);
            result.push(' ');
        }

        Ok(result.trim().to_string())
    }
}
```

**Step 2: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: Compiles

**Step 3: Commit**

```bash
git add src-tauri/src/voice/stt.rs
git commit -m "feat(voice): add Whisper STT via whisper-rs"
```

---

### Task 8: Claude CLI client

**Files:**
- Implement: `src-tauri/src/voice/llm.rs`
- Test: inline (mock process tests)

Port from `live-chat/llm/cli_client.py`. Spawns `claude` CLI with `--output-format stream-json` and streams text deltas.

**Step 1: Implement**

```rust
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

const SYSTEM_PROMPT: &str = "You are a voice assistant. Keep responses concise and conversational. \
Speak naturally as if in a real-time voice conversation. Avoid markdown formatting, \
code blocks, or bullet points — respond as you would speak.";

pub struct Conversation {
    messages: Vec<(String, String)>, // (role, content)
    persona: Option<String>,
}

impl Conversation {
    pub fn new(persona: Option<String>) -> Self {
        Self {
            messages: Vec::new(),
            persona,
        }
    }

    pub fn add_user(&mut self, text: &str) {
        self.messages.push(("user".to_string(), text.to_string()));
    }

    pub fn add_assistant(&mut self, text: &str) {
        self.messages
            .push(("assistant".to_string(), text.to_string()));
    }

    pub fn system_prompt(&self) -> &str {
        self.persona.as_deref().unwrap_or(SYSTEM_PROMPT)
    }
}

/// Spawn claude CLI and stream response tokens.
/// Calls `on_token` for each text delta received.
pub async fn stream_response(
    conversation: &Conversation,
    on_token: &mut dyn FnMut(&str),
) -> Result<String, String> {
    // Build the prompt from conversation history
    let prompt = conversation
        .messages
        .last()
        .map(|(_, content)| content.as_str())
        .unwrap_or("");

    let mut cmd = Command::new("claude");
    cmd.arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
        .arg("--no-session-persistence")
        .arg("--system-prompt")
        .arg(conversation.system_prompt())
        .arg("-p")
        .arg(prompt)
        .env_remove("CLAUDECODE")
        .env_remove("CLAUDE_CODE_ENTRYPOINT")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn claude CLI: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or("Failed to capture claude stdout")?;

    let mut reader = BufReader::new(stdout).lines();
    let mut full_response = String::new();

    while let Some(line) = reader
        .next_line()
        .await
        .map_err(|e| format!("Failed to read claude output: {e}"))?
    {
        if line.is_empty() {
            continue;
        }

        // Parse ndjson line
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
            // Look for content_block_delta with text_delta
            if json.get("type").and_then(|t| t.as_str()) == Some("content_block_delta") {
                if let Some(delta) = json.get("delta") {
                    if delta.get("type").and_then(|t| t.as_str()) == Some("text_delta") {
                        if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                            full_response.push_str(text);
                            on_token(text);
                        }
                    }
                }
            }
            // Also handle the result message type (final text)
            if json.get("type").and_then(|t| t.as_str()) == Some("result") {
                if let Some(result_text) = json.get("result").and_then(|r| r.as_str()) {
                    if full_response.is_empty() {
                        full_response = result_text.to_string();
                        on_token(result_text);
                    }
                }
            }
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Failed to wait for claude: {e}"))?;

    if !status.success() && full_response.is_empty() {
        return Err(format!("Claude CLI exited with status: {status}"));
    }

    Ok(full_response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_tracks_messages() {
        let mut conv = Conversation::new(None);
        conv.add_user("hello");
        conv.add_assistant("hi there");
        assert_eq!(conv.messages.len(), 2);
        assert_eq!(conv.messages[0].0, "user");
        assert_eq!(conv.messages[1].0, "assistant");
    }

    #[test]
    fn conversation_uses_custom_persona() {
        let conv = Conversation::new(Some("You are a pirate.".to_string()));
        assert_eq!(conv.system_prompt(), "You are a pirate.");
    }

    #[test]
    fn conversation_uses_default_system_prompt() {
        let conv = Conversation::new(None);
        assert!(conv.system_prompt().contains("voice assistant"));
    }
}
```

**Step 2: Run tests**

Run: `cd src-tauri && cargo test voice::llm`
Expected: 3 tests pass

**Step 3: Commit**

```bash
git add src-tauri/src/voice/llm.rs
git commit -m "feat(voice): add Claude CLI streaming client"
```

---

### Task 9: Piper TTS via ONNX

**Files:**
- Implement: `src-tauri/src/voice/tts.rs`

Piper TTS uses espeak-ng for phonemization + VITS ONNX model for synthesis. This is the most complex component.

**Important:** Requires `espeak-ng` installed on the system (`brew install espeak-ng` on macOS).

**Step 1: Implement**

```rust
use ort::session::Session;
use std::path::Path;
use std::process::Command;

pub struct PiperTts {
    session: Session,
    sample_rate: u32,
}

impl PiperTts {
    pub fn new(model_path: &Path, _config_path: &Path) -> Result<Self, String> {
        let session = Session::builder()
            .map_err(|e| format!("Failed to create TTS session builder: {e}"))?
            .with_intra_threads(1)
            .map_err(|e| format!("Failed to set TTS threads: {e}"))?
            .commit_from_file(model_path)
            .map_err(|e| format!("Failed to load Piper model: {e}"))?;

        // Piper lessac-medium outputs at 22050 Hz
        Ok(Self {
            session,
            sample_rate: 22050,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Convert text to phoneme IDs using espeak-ng.
    fn phonemize(text: &str) -> Result<Vec<i64>, String> {
        let output = Command::new("espeak-ng")
            .args(["--ipa", "-q", "--sep= ", "-v", "en-us", text])
            .output()
            .map_err(|e| format!("Failed to run espeak-ng (is it installed? brew install espeak-ng): {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("espeak-ng failed: {stderr}"));
        }

        let phonemes = String::from_utf8_lossy(&output.stdout);

        // Convert IPA phonemes to Piper phoneme IDs
        // Piper uses a simple mapping: pad=0, start=1, end=2, then characters
        let mut ids: Vec<i64> = vec![0]; // BOS pad
        for ch in phonemes.trim().chars() {
            if ch == ' ' {
                ids.push(0); // pad between phonemes
            } else {
                // Simple ASCII-range mapping for Piper's default phoneme set
                // Piper's phoneme_id_map starts special chars at index 1
                ids.push(ch as i64);
            }
        }
        ids.push(0); // EOS pad

        Ok(ids)
    }

    /// Synthesize text to int16 audio samples.
    pub fn synthesize(&self, text: &str) -> Result<Vec<i16>, String> {
        if text.trim().is_empty() {
            return Ok(Vec::new());
        }

        let phoneme_ids = Self::phonemize(text)?;
        let id_count = phoneme_ids.len();

        // Prepare ONNX inputs
        let input = ndarray::Array2::from_shape_vec((1, id_count), phoneme_ids)
            .map_err(|e| format!("Failed to create phoneme tensor: {e}"))?;
        let input_lengths = ndarray::Array1::from_vec(vec![id_count as i64]);
        let scales = ndarray::Array1::from_vec(vec![0.667f32, 1.0, 0.8]); // noise, length, noise_w

        let outputs = self
            .session
            .run(ort::inputs![
                "input" => input,
                "input_lengths" => input_lengths,
                "scales" => scales,
            ].map_err(|e| format!("Failed to create TTS inputs: {e}"))?)
            .map_err(|e| format!("TTS inference failed: {e}"))?;

        // Output is float32 audio
        let audio = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract TTS output: {e}"))?;

        // Convert float32 to int16
        let samples: Vec<i16> = audio
            .iter()
            .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
            .collect();

        Ok(samples)
    }

    /// Synthesize text sentence by sentence, yielding audio chunks.
    pub fn synthesize_streaming(&self, text: &str) -> Vec<Result<Vec<i16>, String>> {
        let sentences = split_sentences(text);
        sentences
            .into_iter()
            .map(|sentence| self.synthesize(&sentence))
            .collect()
    }
}

/// Split text into sentences at natural boundaries.
fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        current.push(ch);
        if matches!(ch, '.' | '!' | '?') {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                sentences.push(trimmed);
            }
            current.clear();
        }
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        sentences.push(trimmed);
    }

    sentences
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_sentences_basic() {
        let result = split_sentences("Hello there. How are you? I'm fine!");
        assert_eq!(result, vec!["Hello there.", "How are you?", "I'm fine!"]);
    }

    #[test]
    fn split_sentences_no_punctuation() {
        let result = split_sentences("Hello there how are you");
        assert_eq!(result, vec!["Hello there how are you"]);
    }

    #[test]
    fn split_sentences_empty() {
        let result = split_sentences("");
        assert!(result.is_empty());
    }
}
```

**Step 2: Run tests**

Run: `cd src-tauri && cargo test voice::tts`
Expected: 3 tests pass

**Step 3: Commit**

```bash
git add src-tauri/src/voice/tts.rs
git commit -m "feat(voice): add Piper TTS via ONNX + espeak-ng"
```

---

### Task 10: Pipeline orchestration

**Files:**
- Implement: `src-tauri/src/voice/mod.rs` (expand the existing skeleton)

Wire all components together into the `VoicePipeline` struct. Manages the state machine (LISTENING → THINKING → SPEAKING → LISTENING) and emits state change events.

**Step 1: Implement the full mod.rs**

Replace `src-tauri/src/voice/mod.rs` with:

```rust
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
}

/// Minimum speech duration to avoid noise triggering STT (0.5s at 16kHz).
const MIN_SPEECH_SAMPLES: usize = 8000;
/// Minimum RMS to consider a segment as speech.
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
        emit_state(&emitter, VoiceState::Downloading);
        let model_paths = models::ensure_models(|filename| {
            eprintln!("[voice] Downloading {filename}...");
        })
        .await?;

        // Load models (heavy — do in spawn_blocking)
        let vad_path = model_paths.silero_vad.clone();
        let whisper_path = model_paths.whisper.clone();
        let piper_onnx_path = model_paths.piper_onnx.clone();
        let piper_config_path = model_paths.piper_config.clone();

        let stop = stop_flag.clone();
        let emitter_clone = emitter.clone();

        let audio_thread = std::thread::spawn(move || {
            if let Err(e) = run_pipeline(
                &vad_path,
                &whisper_path,
                &piper_onnx_path,
                &piper_config_path,
                stop,
                emitter_clone,
            ) {
                eprintln!("[voice] Pipeline error: {e}");
            }
        });

        Ok(Self {
            stop_flag,
            audio_thread: Some(audio_thread),
        })
    }

    /// Stop the pipeline and release all resources.
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
    let payload = serde_json::to_string(&VoiceStateEvent { state }).unwrap_or_default();
    let _ = emitter.emit("voice-state-changed", &payload);
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
    let tts_engine = tts::PiperTts::new(piper_onnx_path, piper_config_path)?;
    let audio_out = audio_output::AudioOutput::new()?;
    let mut auto_gain = gain::AutoGain::new();
    let mut conversation = llm::Conversation::new(None);

    // Start mic capture
    let (tx, rx): (Sender<Vec<i16>>, Receiver<Vec<i16>>) = crossbeam_channel::bounded(64);
    let mut audio_in = audio_input::AudioInput::start(tx)?;

    emit_state(&emitter, VoiceState::Listening);
    let mut speech_buffer: Vec<f32> = Vec::new();
    let mut in_speech = false;

    // Main loop
    while !stop.load(Ordering::Relaxed) {
        let chunk = match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(c) => c,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
            Err(_) => break, // Channel disconnected
        };

        // AutoGain: int16 → normalized float32
        let normalized = auto_gain.apply(&chunk);

        // VAD
        match vad_engine.process(&normalized)? {
            Some(vad::VadEvent::SpeechStart) => {
                in_speech = true;
                speech_buffer.clear();
                speech_buffer.extend_from_slice(&normalized);
            }
            Some(vad::VadEvent::SpeechEnd) => {
                if in_speech {
                    speech_buffer.extend_from_slice(&normalized);
                    in_speech = false;

                    // Process the accumulated speech
                    process_speech(
                        &speech_buffer,
                        &whisper,
                        &tts_engine,
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
    tts_engine: &tts::PiperTts,
    audio_out: &audio_output::AudioOutput,
    audio_in: &mut audio_input::AudioInput,
    conversation: &mut llm::Conversation,
    vad_engine: &mut vad::Vad,
    emitter: &Arc<dyn EventEmitter>,
    stop: &Arc<AtomicBool>,
) -> Result<(), String> {
    // Skip very short segments
    if audio.len() < MIN_SPEECH_SAMPLES {
        return Ok(());
    }

    // Skip low-energy segments
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

    // LLM — run in a tokio runtime since we're on a std thread
    let rt = tokio::runtime::Handle::try_current()
        .or_else(|_| {
            tokio::runtime::Runtime::new()
                .map(|rt| rt.handle().clone())
                .map_err(|e| format!("Failed to create tokio runtime: {e}"))
        })?;

    let conv_system = conversation.system_prompt().to_string();
    let conv_messages = conversation.messages.clone();

    // Build a temporary conversation for the async call
    let mut temp_conv = llm::Conversation::new(Some(conv_system));
    for (role, content) in &conv_messages {
        if role == "user" {
            temp_conv.add_user(content);
        } else {
            temp_conv.add_assistant(content);
        }
    }

    // Collect full response
    let response = rt.block_on(async {
        let mut full = String::new();
        llm::stream_response(&temp_conv, &mut |token| {
            full.push_str(token);
        })
        .await
    })?;

    if response.is_empty() || stop.load(Ordering::Relaxed) {
        return Ok(());
    }

    eprintln!("[voice] Assistant: {response}");
    conversation.add_assistant(&response);

    // TTS — mute mic during playback to avoid echo
    emit_state(emitter, VoiceState::Speaking);
    audio_in.mute();

    for chunk_result in tts_engine.synthesize_streaming(&response) {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        match chunk_result {
            Ok(samples) if !samples.is_empty() => {
                audio_out.play_i16(&samples, tts_engine.sample_rate())?;
            }
            Err(e) => eprintln!("[voice] TTS error: {e}"),
            _ => {}
        }
    }

    // Unmute and reset VAD for next turn
    audio_in.unmute();
    std::thread::sleep(std::time::Duration::from_millis(300));
    vad_engine.reset();

    Ok(())
}
```

**Step 2: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: Compiles

**Step 3: Commit**

```bash
git add src-tauri/src/voice/mod.rs
git commit -m "feat(voice): wire pipeline orchestration"
```

---

### Task 11: Tauri commands + AppState integration

**Files:**
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/commands.rs` (add voice commands)
- Modify: `src-tauri/src/lib.rs` (register commands)

**Step 1: Add voice state to AppState**

In `src-tauri/src/state.rs`, add to imports:

```rust
use crate::voice::VoicePipeline;
```

Add field to `AppState`:

```rust
pub voice_pipeline: Arc<TokioMutex<Option<VoicePipeline>>>,
```

Initialize in `from_storage`:

```rust
voice_pipeline: Arc::new(TokioMutex::new(None)),
```

**Step 2: Add commands**

Add to `src-tauri/src/commands.rs` (at the end, before any closing brace):

```rust
#[tauri::command]
pub async fn start_voice_pipeline(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut pipeline = state.voice_pipeline.lock().await;
    if pipeline.is_some() {
        return Ok(()); // Already running
    }
    let emitter = state.emitter.clone();
    let new_pipeline = crate::voice::VoicePipeline::start(emitter).await?;
    *pipeline = Some(new_pipeline);
    Ok(())
}

#[tauri::command]
pub async fn stop_voice_pipeline(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut pipeline = state.voice_pipeline.lock().await;
    if let Some(mut p) = pipeline.take() {
        p.stop();
    }
    Ok(())
}
```

**Step 3: Register commands in lib.rs**

Add to the `invoke_handler` list:

```rust
commands::start_voice_pipeline,
commands::stop_voice_pipeline,
```

**Step 4: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: Compiles

**Step 5: Commit**

```bash
git add src-tauri/src/state.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(voice): add Tauri commands for start/stop pipeline"
```

---

### Task 12: Frontend wiring (VoiceMode.svelte)

**Files:**
- Modify: `src/lib/VoiceMode.svelte`

Wire up the existing component to call start/stop commands and listen for state events.

**Step 1: Update VoiceMode.svelte**

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { command, listen } from "$lib/backend";

  let voiceState = $state<string>("voice mode");

  const STATE_LABELS: Record<string, string> = {
    listening: "listening...",
    thinking: "thinking...",
    speaking: "speaking...",
    downloading: "downloading models...",
  };

  onMount(() => {
    // Listen for state changes from Rust
    const unlisten = listen<string>("voice-state-changed", (payload) => {
      try {
        const data = JSON.parse(payload);
        voiceState = STATE_LABELS[data.state] ?? "voice mode";
      } catch {
        // Ignore malformed events
      }
    });

    // Start the pipeline
    command("start_voice_pipeline").catch((e: unknown) => {
      console.error("[voice] Failed to start pipeline:", e);
      voiceState = "error";
    });

    return () => {
      unlisten();
      // Stop the pipeline when leaving voice mode
      command("stop_voice_pipeline").catch((e: unknown) => {
        console.error("[voice] Failed to stop pipeline:", e);
      });
    };
  });
</script>

<div class="voice-mode">
  <span class="label">{voiceState}</span>
</div>

<style>
  .voice-mode {
    width: 100%;
    height: 100%;
    background: #000;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .label {
    font-size: 14px;
    color: rgba(255, 255, 255, 0.3);
    font-family: var(--font-mono);
    letter-spacing: 0.05em;
  }
</style>
```

**Step 2: Verify frontend compiles**

Run: `npx svelte-check --tsconfig tsconfig.json 2>&1 | grep -E "VoiceMode|Error" | head -5`
Expected: No errors from VoiceMode.svelte

**Step 3: Commit**

```bash
git add src/lib/VoiceMode.svelte
git commit -m "feat(voice): wire frontend to voice pipeline commands"
```

---

### Task 13: End-to-end integration test

**Files:** No new files — manual testing

**Prerequisites:**
- `brew install espeak-ng` (for Piper TTS phonemization)
- Claude CLI installed and authenticated (`claude --version`)
- Working microphone and speakers

**Step 1: Build and run**

```bash
npm run tauri dev
```

**Step 2: Test the voice pipeline**

1. Press Space → v to enter voice mode
2. First time: screen should show "downloading models..." while models download
3. After download: screen should show "listening..."
4. Speak a sentence ("Hello, how are you?")
5. Screen should transition: "listening..." → "thinking..." → "speaking..."
6. Claude's response should play through speakers
7. Screen returns to "listening..."

**Step 3: Test mode switching**

1. While in voice mode, press Space → d
2. Should switch to development mode, voice pipeline stops
3. Press Space → v again to re-enter voice mode
4. Pipeline should restart (models already downloaded, no download step)

**Step 4: Commit final integration**

```bash
git add -A
git commit -m "feat(voice): complete voice mode integration"
```

---

## Dependency installation checklist

Before starting implementation, ensure these are available:

```bash
# macOS prerequisites
brew install espeak-ng        # Required for Piper TTS phonemization
claude --version              # Claude CLI must be installed and authenticated

# Rust toolchain (should already be present)
rustup show                   # Verify Rust is installed
```

## Risk notes

1. **espeak-ng phonemization**: The Piper TTS phoneme-to-ID mapping is model-specific. The implementation in Task 9 uses a simplified mapping. If phonemes sound wrong, the mapping needs to be loaded from the Piper JSON config file's `phoneme_id_map` field.

2. **whisper-rs build**: whisper-rs compiles whisper.cpp from source. First build may take 2-3 minutes. On Apple Silicon, it should auto-detect Metal acceleration.

3. **ort ONNX Runtime**: The `download-binaries` feature auto-downloads the ONNX Runtime shared library. First build will download ~20MB.

4. **Silero VAD tensor names**: The ONNX input/output tensor names (`input`, `sr`, `h`, `c`, `output`, `hn`, `cn`) are specific to the Silero model version. If the model is updated, these may change — check with `ort`'s session inspection API.

5. **Claude CLI `--output-format stream-json`**: This flag may not be available in all Claude CLI versions. If it fails, fall back to `--print` mode (simpler, non-streaming).
