# Voice Mode Design

## Overview

Add voice chat with Claude to The Controller. Full local pipeline — no paid API, uses Claude CLI (free with subscription). Triggered by space+v, shows minimal state indicator on black screen.

## Pipeline

```
cpal mic → AutoGain → Silero VAD → whisper.cpp STT → Claude CLI → Piper TTS → cpal speaker
```

All processing happens in Rust. Frontend only displays state.

## Architecture

```
┌─────────────────────────────────────────────────┐
│  Frontend (Svelte)                              │
│                                                 │
│  VoiceMode.svelte                               │
│  - Shows: "listening..." / "thinking..." /      │
│           "speaking..." / "downloading..."      │
│  - onMount → invoke start_voice_pipeline        │
│  - onDestroy → invoke stop_voice_pipeline       │
│  - Listens to event: "voice-state-changed"      │
│                                                 │
├─────────────────────────────────────────────────┤
│  Backend (Rust)                                 │
│                                                 │
│  voice/mod.rs — pipeline orchestration          │
│  voice/audio_input.rs — cpal mic, 16kHz mono    │
│  voice/audio_output.rs — cpal speaker playback  │
│  voice/vad.rs — Silero VAD via ort (ONNX)       │
│  voice/stt.rs — whisper-rs (whisper.cpp)        │
│  voice/tts.rs — Piper TTS via ONNX             │
│  voice/llm.rs — Claude CLI subprocess           │
│  voice/gain.rs — AutoGain normalization         │
│  voice/models.rs — model download & paths       │
└─────────────────────────────────────────────────┘
```

## State Machine

```
         ┌──────────┐
         │   IDLE   │  ← stop_voice_pipeline
         └────┬─────┘
              │ start_voice_pipeline
              ▼
         ┌──────────┐
    ┌───►│LISTENING │ ←──────────────────┐
    │    └────┬─────┘                    │
    │         │ VAD detects speech end   │
    │         ▼                          │
    │    ┌──────────┐                    │
    │    │THINKING  │                    │
    │    └────┬─────┘                    │
    │         │ Claude CLI responds      │
    │         ▼                          │
    │    ┌──────────┐                    │
    │    │SPEAKING  │────────────────────┘
    │    └──────────┘  TTS playback done
    │
    └── (mic stays open, VAD resets)
```

## Data Flow

1. **LISTENING**: cpal captures 16kHz mono chunks (~30ms). AutoGain normalizes. Silero VAD detects speech boundaries. Speech chunks accumulate in buffer. On speech end → THINKING.

2. **THINKING**: Concatenated speech buffer → whisper.cpp → text. Text piped to Claude CLI stdin. Response streamed from stdout. Mic chunks discarded. → SPEAKING as sentences arrive.

3. **SPEAKING**: Piper TTS synthesizes response sentence-by-sentence (streamed — speak while still generating). Audio played via cpal output. When done → LISTENING.

## Tauri Commands

```rust
#[tauri::command]
async fn start_voice_pipeline(app: AppHandle) -> Result<(), String>

#[tauri::command]
async fn stop_voice_pipeline(app: AppHandle) -> Result<(), String>
```

## Tauri Events

```
"voice-state-changed" → { state: "listening" | "thinking" | "speaking" | "downloading" }
```

## Frontend

VoiceMode.svelte displays a single centered label on black background. The label reflects the current pipeline state. No transcript, no other UI elements.

## Rust Dependencies

```toml
cpal = "0.15"          # Audio I/O
whisper-rs = "0.12"    # Speech-to-text
ort = "2"              # ONNX Runtime (Silero VAD + Piper TTS)
hound = "3.5"          # WAV encoding for whisper input
```

## Thread Model

- cpal audio callbacks run on a dedicated OS thread (real-time, can't use async)
- VAD/STT/TTS run on tokio blocking tasks
- Claude CLI is async (tokio::process)
- Communication via crossbeam channels

## Model Files

Stored in `~/.the-controller/voice-models/`. Downloaded on first use.

| Model | Size | Purpose |
|-------|------|---------|
| Silero VAD | ~2MB | Voice activity detection |
| Whisper base | ~75MB | Speech-to-text |
| Piper en_US-lessac-medium | ~30MB | Text-to-speech |

## Pipeline Ownership

`VoicePipeline` struct stored in `tauri::State<Mutex<Option<VoicePipeline>>>`. `start_voice_pipeline` creates and starts it. `stop_voice_pipeline` drops it, releasing cpal streams and killing the Claude CLI subprocess.

## Constraints

- Claude subscription only, no paid API — uses Claude CLI backend
- macOS primary target (Apple Silicon for whisper.cpp acceleration)
- All models run locally — no network calls except Claude CLI
