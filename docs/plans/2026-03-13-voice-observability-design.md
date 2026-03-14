# Voice Mode Observability + Closed-Loop Testing Design

## Overview

Add debug and transcript panels to voice mode for pipeline observability, plus an automated e2e test using a pre-recorded speech WAV.

## Observability Panes

Voice mode gets two toggleable side panels on a black background:

```
┌──────────────────┬────────────┬──────────────────┐
│ Debug (d)        │            │ Transcript (t)   │
│                  │ listening  │                  │
│ mic rms=312      │    ...     │ You: hello       │
│ vad prob=0.003   │            │ AI: hey there    │
│ mic rms=847      │            │                  │
│ vad prob=0.067   │            │                  │
└──────────────────┴────────────┴──────────────────┘
```

- **Default**: Both hidden (clean black screen)
- **`d`**: Toggle left debug panel
- **`t`**: Toggle right transcript panel
- **State label**: Always centered in remaining space

### Debug Log Entries

Streamed via `voice-debug` Tauri event. Throttled to ~2/sec for periodic data, immediate for events.

- `mic rms=XXX` — raw audio level (confirms mic captures)
- `vad prob=X.XXX` — VAD confidence (confirms model processes)
- `speech_start` / `speech_end` — VAD boundary events
- `stt: "transcribed text"` — STT result
- `llm: streaming...` / `llm: done` — Claude CLI status
- `tts: synthesizing...` / `tts: playing` — TTS status
- `error: ...` — pipeline errors

### Transcript Entries

Streamed via `voice-transcript` Tauri event.

- `{ role: "user", text: "..." }` — when STT completes
- `{ role: "assistant", text: "..." }` — when LLM responds

## Closed-Loop E2E Test

Bundle a pre-recorded WAV file (~3s of real human speech) in the repo at `src-tauri/test-data/speech_sample.wav`.

Test flow:
```
WAV file → VAD → STT → LLM → TTS → verify audio output
```

Assertions:
1. VAD fires SpeechStart + SpeechEnd
2. STT produces non-empty text matching expected content
3. LLM returns a non-empty response
4. TTS produces non-empty audio samples

## Backend Changes

- Pipeline emits `voice-debug` events at key points
- Pipeline emits `voice-transcript` events for user/assistant messages
- Throttle: rms/prob every ~500ms (~15 chunks), speech/error immediate

## Frontend Changes

- `VoiceMode.svelte`: Two optional side panels, toggled by local state
- `HotkeyManager.svelte`: Route `d` and `t` keys to voice mode when active
- Debug panel: scrolling monospace log, auto-scroll, max ~200 lines
- Transcript panel: chat-style messages, auto-scroll
