# Voice Observability + Closed-Loop Testing Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Add debug/transcript side panels to voice mode and a pre-recorded WAV e2e test for the voice pipeline.

**Architecture:** Pipeline emits `voice-debug` and `voice-transcript` Tauri events. Frontend toggles left (debug) and right (transcript) panels via `d`/`t` keys. E2E test feeds a bundled WAV through VAD → STT → LLM → TTS and asserts each stage succeeds.

**Tech Stack:** Svelte 5 (frontend panels), Rust Tauri events (backend telemetry), hound (WAV reading for test)

**Design doc:** `docs/plans/2026-03-13-voice-observability-design.md`

---

### Task 1: Record speech sample WAV

**Files:**
- Create: `src-tauri/test-data/speech_sample.wav`

We need a real human speech WAV file for the e2e test. Use the existing TTS to generate speech, then record it through the speaker→mic path to get a "real" audio sample. Alternatively, use `say` (macOS) piped to a file, which produces natural-sounding speech that Silero VAD recognizes.

**Step 1: Generate a speech WAV using macOS `say`**

```bash
cd /Users/noel/.the-controller/worktrees/the-controller/session-2-1bc4eb
mkdir -p src-tauri/test-data
say -o src-tauri/test-data/speech_sample_44k.aiff "Hello, how are you doing today?"
ffmpeg -i src-tauri/test-data/speech_sample_44k.aiff -ar 16000 -ac 1 -f wav src-tauri/test-data/speech_sample.wav -y
rm src-tauri/test-data/speech_sample_44k.aiff
```

If `ffmpeg` is not available, use `sox` or `afconvert`:
```bash
say -o src-tauri/test-data/speech_sample_44k.aiff "Hello, how are you doing today?"
afconvert -f WAVE -d LEI16@16000 -c 1 src-tauri/test-data/speech_sample_44k.aiff src-tauri/test-data/speech_sample.wav
rm src-tauri/test-data/speech_sample_44k.aiff
```

**Step 2: Verify the WAV file**

```bash
file src-tauri/test-data/speech_sample.wav
# Expected: RIFF (little-endian) data, WAVE audio, Microsoft PCM, 16 bit, mono 16000 Hz
```

**Step 3: Commit**

```bash
git add src-tauri/test-data/speech_sample.wav
git commit -m "test(voice): add speech sample WAV for e2e testing"
```

---

### Task 2: Add debug event emission to pipeline

**Files:**
- Modify: `src-tauri/src/voice/mod.rs`

Add a helper function `emit_debug` and instrument the pipeline loop and `process_speech` with debug events. Throttle periodic data (rms, vad prob) to every ~15 chunks (~500ms).

**Step 1: Add emit_debug helper and instrument run_pipeline**

In `src-tauri/src/voice/mod.rs`, add after the `emit_state` function:

```rust
fn emit_debug(emitter: &Arc<dyn EventEmitter>, msg: &str) {
    let payload = serde_json::json!({
        "ts": chrono::Local::now().format("%H:%M:%S").to_string(),
        "msg": msg,
    })
    .to_string();
    let _ = emitter.emit("voice-debug", &payload);
}
```

Then in `run_pipeline`, add a chunk counter and periodic debug emission:

After `let mut in_speech = false;` add:
```rust
let mut chunk_count: u64 = 0;
```

After `let normalized = auto_gain.apply(&chunk);` add:
```rust
chunk_count += 1;

// Emit debug info every ~500ms (15 chunks at 16kHz/512)
if chunk_count % 15 == 0 {
    let rms_i16 = (chunk.iter().map(|&s| (s as f32) * (s as f32)).sum::<f32>()
        / chunk.len() as f32)
        .sqrt();
    let prob = vad_engine.last_prob();
    emit_debug(&emitter, &format!("mic rms={:.0} vad prob={:.3}", rms_i16, prob));
}
```

On `SpeechStart`:
```rust
emit_debug(&emitter, "speech_start");
```

On `SpeechEnd`:
```rust
emit_debug(&emitter, &format!("speech_end ({:.1}s)", speech_buffer.len() as f32 / 16000.0));
```

In `process_speech`, add debug emissions:
- After STT: `emit_debug(emitter, &format!("stt: \"{}\"", text));`
- Before LLM: `emit_debug(emitter, "llm: streaming...");`
- After LLM: `emit_debug(emitter, "llm: done");`
- Before TTS: `emit_debug(emitter, "tts: synthesizing...");`
- After TTS playback: `emit_debug(emitter, "tts: done");`
- On errors: `emit_debug(emitter, &format!("error: {}", e));`

Also add `use chrono::Local;` or use `chrono` at the call site. `chrono` is already in `Cargo.toml`.

**Step 2: Add transcript event emission**

In `process_speech`, after successful STT:
```rust
let transcript_payload = serde_json::json!({
    "role": "user",
    "text": text,
}).to_string();
let _ = emitter.emit("voice-transcript", &transcript_payload);
```

After successful LLM response:
```rust
let transcript_payload = serde_json::json!({
    "role": "assistant",
    "text": response,
}).to_string();
let _ = emitter.emit("voice-transcript", &transcript_payload);
```

**Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: Compiles

**Step 4: Commit**

```bash
git add src-tauri/src/voice/mod.rs
git commit -m "feat(voice): emit debug and transcript events from pipeline"
```

---

### Task 3: Frontend — VoiceMode with debug and transcript panels

**Files:**
- Modify: `src/lib/VoiceMode.svelte`

Replace the current simple component with a three-column layout: optional left debug panel, centered state label, optional right transcript panel.

**Step 1: Implement the updated VoiceMode.svelte**

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { command, listen } from "$lib/backend";

  let voiceState = $state<string>("voice mode");
  let showDebug = $state(false);
  let showTranscript = $state(false);
  let debugLog = $state<string[]>([]);
  let transcript = $state<{ role: string; text: string }[]>([]);

  const MAX_DEBUG_LINES = 200;

  const STATE_LABELS: Record<string, string> = {
    listening: "listening...",
    thinking: "thinking...",
    speaking: "speaking...",
  };

  export function toggleDebug() {
    showDebug = !showDebug;
  }

  export function toggleTranscript() {
    showTranscript = !showTranscript;
  }

  let debugEl: HTMLElement | undefined = $state();
  let transcriptEl: HTMLElement | undefined = $state();

  $effect(() => {
    if (debugEl && debugLog.length) {
      debugEl.scrollTop = debugEl.scrollHeight;
    }
  });

  $effect(() => {
    if (transcriptEl && transcript.length) {
      transcriptEl.scrollTop = transcriptEl.scrollHeight;
    }
  });

  onMount(() => {
    const unlistenState = listen<string>("voice-state-changed", (payload) => {
      try {
        const data = JSON.parse(payload);
        if (data.state === "downloading" && data.filename) {
          const pct = data.percent != null ? ` ${data.percent}%` : "";
          voiceState = `downloading ${data.filename}${pct}`;
        } else if (data.state === "error") {
          voiceState = `error: ${data.error ?? "unknown"}`;
        } else {
          voiceState = STATE_LABELS[data.state] ?? "voice mode";
        }
      } catch {
        // Ignore malformed events
      }
    });

    const unlistenDebug = listen<string>("voice-debug", (payload) => {
      try {
        const data = JSON.parse(payload);
        const line = `[${data.ts}] ${data.msg}`;
        debugLog = [...debugLog.slice(-(MAX_DEBUG_LINES - 1)), line];
      } catch {
        // Ignore
      }
    });

    const unlistenTranscript = listen<string>("voice-transcript", (payload) => {
      try {
        const data = JSON.parse(payload);
        transcript = [...transcript, { role: data.role, text: data.text }];
      } catch {
        // Ignore
      }
    });

    command("start_voice_pipeline").catch((e: unknown) => {
      console.error("[voice] Failed to start pipeline:", e);
      voiceState = "error";
    });

    return () => {
      unlistenState();
      unlistenDebug();
      unlistenTranscript();
      command("stop_voice_pipeline").catch((e: unknown) => {
        console.error("[voice] Failed to stop pipeline:", e);
      });
    };
  });
</script>

<div class="voice-mode">
  {#if showDebug}
    <div class="panel debug-panel" bind:this={debugEl}>
      <div class="panel-header">debug</div>
      <div class="panel-content">
        {#each debugLog as line}
          <div class="log-line">{line}</div>
        {/each}
      </div>
    </div>
  {/if}

  <div class="center">
    <span class="label">{voiceState}</span>
  </div>

  {#if showTranscript}
    <div class="panel transcript-panel" bind:this={transcriptEl}>
      <div class="panel-header">transcript</div>
      <div class="panel-content">
        {#each transcript as entry}
          <div class="transcript-entry" class:user={entry.role === "user"} class:assistant={entry.role === "assistant"}>
            <span class="role">{entry.role === "user" ? "You" : "AI"}:</span>
            <span class="text">{entry.text}</span>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .voice-mode {
    width: 100%;
    height: 100%;
    background: #000;
    display: flex;
    overflow: hidden;
  }

  .center {
    flex: 1;
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

  .panel {
    width: 300px;
    display: flex;
    flex-direction: column;
    border-color: rgba(255, 255, 255, 0.1);
    overflow-y: auto;
  }

  .debug-panel {
    border-right: 1px solid rgba(255, 255, 255, 0.1);
  }

  .transcript-panel {
    border-left: 1px solid rgba(255, 255, 255, 0.1);
  }

  .panel-header {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.2);
    font-family: var(--font-mono);
    padding: 8px 12px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }

  .panel-content {
    flex: 1;
    padding: 0 12px 12px;
    overflow-y: auto;
  }

  .log-line {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.4);
    font-family: var(--font-mono);
    line-height: 1.6;
    white-space: nowrap;
  }

  .transcript-entry {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.5);
    font-family: var(--font-mono);
    margin-bottom: 12px;
    line-height: 1.5;
  }

  .transcript-entry.user .role {
    color: rgba(137, 220, 235, 0.7);
  }

  .transcript-entry.assistant .role {
    color: rgba(203, 166, 247, 0.7);
  }

  .role {
    font-weight: 600;
    margin-right: 6px;
  }

  .text {
    color: rgba(255, 255, 255, 0.6);
  }
</style>
```

**Step 2: Verify frontend compiles**

Run: `npx svelte-check --tsconfig tsconfig.json 2>&1 | grep -E "VoiceMode|Error" | head -5`
Expected: No errors from VoiceMode.svelte

**Step 3: Commit**

```bash
git add src/lib/VoiceMode.svelte
git commit -m "feat(voice): add debug and transcript side panels"
```

---

### Task 4: Hotkey routing for d/t in voice mode

**Files:**
- Modify: `src/lib/HotkeyManager.svelte`
- Modify: `src/App.svelte` (to get ref to VoiceMode)

The `d` and `t` keys need to toggle the debug and transcript panels when in voice mode. These keys are already used in other modes (`d` = delete in development), so we need mode-specific routing.

**Step 1: Add voice mode key handling to HotkeyManager.svelte**

In `HotkeyManager.svelte`, the hotkey dispatch needs to check if we're in voice mode and route `d`/`t` accordingly. The simplest approach: add a check before `handleHotkey(e.key)` for voice-mode-specific keys.

After the agent-panel block (line ~573) and before `// Try to handle as hotkey`, add:

```typescript
// Voice mode: d = debug, t = transcript
if (currentMode === "voice") {
  if (e.key === "d" || e.key === "t") {
    e.stopPropagation();
    e.preventDefault();
    dispatchAction({ type: "voice-toggle-panel", panel: e.key === "d" ? "debug" : "transcript" });
    pushKeystroke(e.key);
    return;
  }
}
```

**Step 2: Add the HotkeyAction type**

In `src/lib/stores.ts`, add to the `HotkeyAction` union type:

```typescript
| { type: "voice-toggle-panel"; panel: "debug" | "transcript" }
```

**Step 3: Handle the action in App.svelte**

In `App.svelte`, we need a ref to VoiceMode and handle the action. Add a `let voiceModeRef` and use `bind:this`:

In the script section, add:
```typescript
let voiceModeRef: { toggleDebug: () => void; toggleTranscript: () => void } | undefined = $state();
```

In the template, change `<VoiceMode />` to:
```svelte
<VoiceMode bind:this={voiceModeRef} />
```

In the `$effect` that subscribes to `hotkeyAction`, add:
```typescript
} else if (action?.type === "voice-toggle-panel") {
  if (action.panel === "debug") {
    voiceModeRef?.toggleDebug();
  } else {
    voiceModeRef?.toggleTranscript();
  }
}
```

**Step 4: Verify**

Run: `npx svelte-check --tsconfig tsconfig.json 2>&1 | grep Error | head -5`
Expected: No new errors

Run: `npx vitest run`
Expected: All tests pass

**Step 5: Commit**

```bash
git add src/lib/HotkeyManager.svelte src/lib/stores.ts src/App.svelte
git commit -m "feat(voice): route d/t keys to toggle debug/transcript panels"
```

---

### Task 5: Closed-loop E2E test

**Files:**
- Create: `src-tauri/tests/voice_e2e.rs`

Integration test that feeds the bundled WAV file through VAD → STT → LLM → TTS and asserts each stage.

**Step 1: Write the test**

Create `src-tauri/tests/voice_e2e.rs`:

```rust
//! E2E voice pipeline test using a pre-recorded speech WAV.
//! Feeds audio through: WAV → VAD → STT → LLM → TTS
//!
//! Run with: cargo test --test voice_e2e -- --nocapture
//! Requires: models downloaded (~/.the-controller/voice-models/), claude CLI installed

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
        hound::SampleFormat::Float => {
            reader.samples::<f32>().map(|s| s.unwrap()).collect()
        }
    }
}

#[test]
fn test_voice_pipeline_e2e() {
    let wav_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-data/speech_sample.wav");
    if !wav_path.exists() {
        eprintln!("Skipping: speech_sample.wav not found. Run Task 1 to generate it.");
        return;
    }

    let paths = the_controller_lib::voice::models::ModelPaths::new();
    if !paths.all_present() {
        eprintln!("Skipping: voice models not downloaded. Run the app once first.");
        return;
    }

    // 1. Read WAV
    let audio = read_wav_f32(&wav_path);
    assert!(audio.len() > 16000, "WAV too short (need >1s)");
    println!("WAV: {} samples ({:.1}s)", audio.len(), audio.len() as f32 / 16000.0);

    // 2. Feed through VAD
    let mut vad = the_controller_lib::voice::vad::Vad::new(&paths.silero_vad, 800)
        .expect("Failed to load VAD");

    // Pad with silence for clean boundaries
    let silence = vec![0.0f32; 8000]; // 0.5s
    let padded: Vec<f32> = silence.iter()
        .chain(audio.iter())
        .chain(silence.iter())
        .copied()
        .collect();

    let mut speech_segments: Vec<Vec<f32>> = Vec::new();
    let mut current_segment: Vec<f32> = Vec::new();
    let mut in_speech = false;

    for chunk in padded.chunks(512) {
        if chunk.len() < 512 { break; }
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
    assert!(!speech_segments.is_empty(), "VAD detected no speech in WAV — check the WAV file has audible speech at 16kHz");

    // 3. STT
    let whisper = the_controller_lib::voice::stt::WhisperStt::new(&paths.whisper)
        .expect("Failed to load Whisper");

    let speech_audio = &speech_segments[0];
    let text = whisper.transcribe(speech_audio).expect("STT failed");
    println!("STT: \"{}\"", text);
    assert!(!text.is_empty(), "STT produced empty transcription");

    // 4. LLM
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut conv = the_controller_lib::voice::llm::Conversation::new(None);
    conv.add_user(&text);

    let response = rt.block_on(async {
        let mut full = String::new();
        the_controller_lib::voice::llm::stream_response(&conv, &mut |token| {
            full.push_str(token);
        })
        .await
        .map(|_| full)
    })
    .expect("LLM failed");

    println!("LLM: \"{}\"", response);
    assert!(!response.is_empty(), "LLM produced empty response");

    // 5. TTS
    let mut tts = the_controller_lib::voice::tts::PiperTts::new(&paths.piper_onnx, &paths.piper_config)
        .expect("Failed to load TTS");

    let tts_audio = tts.synthesize(&response).expect("TTS failed");
    println!("TTS: {} samples ({:.1}s at {}Hz)", tts_audio.len(), tts_audio.len() as f32 / tts.sample_rate() as f32, tts.sample_rate());
    assert!(!tts_audio.is_empty(), "TTS produced no audio");

    println!("\n=== E2E PASS: WAV → VAD → STT → LLM → TTS ===");
}
```

**Step 2: Run the test**

Run: `cd src-tauri && cargo test --test voice_e2e -- --nocapture`
Expected: All assertions pass, prints E2E PASS

**Step 3: Commit**

```bash
git add src-tauri/tests/voice_e2e.rs
git commit -m "test(voice): add closed-loop e2e pipeline test"
```

---

### Task 6: Verify everything works together

**Files:** No new files — integration verification

**Step 1: Run all tests**

```bash
cd /Users/noel/.the-controller/worktrees/the-controller/session-2-1bc4eb
npx vitest run
cd src-tauri && cargo test
cd src-tauri && cargo test --test voice_e2e -- --nocapture
```

Expected: All pass

**Step 2: Manual verification**

```bash
npm run tauri dev
```

1. Press Space → v (enter voice mode)
2. Press `d` — left debug panel should appear with scrolling log lines
3. Verify `mic rms=XXX` lines appear (confirms audio capture)
4. Verify `vad prob=X.XXX` lines appear (confirms VAD processing)
5. Press `t` — right transcript panel should appear (empty until speech)
6. Speak into mic — watch debug panel for `speech_start` / `speech_end`
7. If speech detected: transcript panel shows "You: ..." and "AI: ..."
8. Press `d` again to hide debug, `t` to hide transcript
9. Press Space → d to leave voice mode

**Step 3: Final commit**

```bash
git add -A
git commit -m "feat(voice): complete observability panes and e2e test"
```

---

## Risk Notes

1. **WAV generation**: The `say` command on macOS generates AIFF, needs conversion to 16kHz mono WAV. If `ffmpeg` isn't available, `afconvert` (built-in macOS) works as fallback.

2. **VAD on macOS `say` audio**: macOS `say` produces clean synthetic speech. If Silero VAD doesn't detect it (similar to Piper TTS issue), try recording actual human speech via `rec` (sox) instead:
   ```bash
   brew install sox
   rec -r 16000 -c 1 -b 16 src-tauri/test-data/speech_sample.wav trim 0 3
   ```

3. **E2E test requires Claude CLI**: The LLM step calls `claude` CLI. In CI without Claude CLI, the test gracefully skips. For local development, Claude CLI must be installed and authenticated.

4. **Svelte `bind:this` with `export function`**: The VoiceMode component exports `toggleDebug`/`toggleTranscript` functions that App.svelte calls via a component ref. This is the standard Svelte 5 pattern for imperative child methods.
