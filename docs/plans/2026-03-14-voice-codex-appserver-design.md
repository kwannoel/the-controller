# Voice Pipeline: Switch LLM Backend to Codex App-Server

## Problem

Voice mode reply latency is bottlenecked by the LLM step. The current implementation spawns a fresh `claude` CLI process per turn (~0.5-1s startup overhead) and relies on `--continue` for session continuity. Shuo (github.com/NickTikhonov/shuo) demonstrates that a persistent LLM connection with token streaming achieves sub-500ms time-to-first-audio.

## Decision

Replace `claude` CLI with a persistent `codex app-server` process. The app-server uses the Codex Pro subscription (chatgpt auth, no API key), streams token deltas via JSON-RPC 2.0 over stdio, and manages conversation history server-side.

## Scope

Pure LLM backend swap. No changes to VAD, STT, TTS, audio I/O, barge-in detection, sentence splitting, or frontend.

## Architecture

### CodexAppServer Struct

```rust
pub struct CodexAppServer {
    child: tokio::process::Child,
    stdin: tokio::io::BufWriter<ChildStdin>,
    stdout: tokio::io::BufReader<ChildStdout>,
    thread_id: String,
    next_id: u64,
    current_turn_id: Option<String>,
}
```

### Public API

- `async fn start(system_prompt: &str) -> Result<Self, String>` — spawn process, handshake, thread/start
- `async fn stream_response(&mut self, text: &str, on_token: &mut dyn FnMut(&str)) -> Result<String, String>` — send turn/start, stream deltas, return full text
- `async fn cancel_turn(&mut self) -> Result<(), String>` — send turn/interrupt for barge-in
- Drop kills child process

### Lifecycle

1. Pipeline starts → `CodexAppServer::start(system_prompt)` spawns `codex app-server` via stdio
2. Handshake: `initialize` → `initialized` → `thread/start`
3. Each user utterance: `turn/start` → read `item/agentMessage/delta` events → `on_token()` callback → `turn/completed`
4. Barge-in: `turn/interrupt` → server acknowledges → `turn/completed` with status `"interrupted"`
5. Pipeline stops → child process killed on drop

## Protocol

### Initialization (once)

```
→ initialize (id:0)     clientInfo: {name: "voice-pipeline", version: "0.1.0"}
← response              server ready
→ initialized           notification
→ thread/start (id:1)   model: "gpt-5.3-codex-spark", ephemeral: true,
                         approvalPolicy: "never", sandbox: "danger-full-access",
                         baseInstructions: <system prompt>, personality: "friendly"
← response              thread_id returned
```

### Each Turn

```
→ turn/start (id:N)     threadId, input: [{type: "text", text: <stt>}], effort: "low"
← item/agentMessage/delta × N    on_token() per delta
← item/completed        full text
← turn/completed        done
```

### Barge-In

```
→ turn/interrupt (id:N) threadId, turnId
← turn/completed        status: "interrupted"
```

## Changes

### llm.rs — Rewrite

- Remove `claude` CLI spawning, stream-json parsing, `live_chat_dir()`
- Add `CodexAppServer` struct with JSON-RPC 2.0 over stdin/stdout
- `stream_response()` sends `turn/start`, reads notifications line-by-line, calls `on_token` for `item/agentMessage/delta` events
- `cancel_turn()` sends `turn/interrupt`
- `Conversation` struct retained for transcript bookkeeping only (no longer constructs prompts)

### mod.rs — Minimal Changes

- `run_pipeline()` spawns `CodexAppServer::start()` at init
- `SpeechContext` holds `CodexAppServer` instead of `Conversation`
- `process_speech()` calls `app_server.stream_response(text, on_token)`
- Barge-in calls `app_server.cancel_turn()`
- Sentence splitting, TTS, VAD, audio — unchanged

### No frontend changes

`VoiceMode.svelte` consumes the same events.

## Configuration

- **Model:** `gpt-5.3-codex-spark` (configurable, ultra-fast)
- **Effort:** `low` (minimum latency)
- **Auth:** Codex Pro subscription via `~/.codex/auth.json` (chatgpt mode, no API key)
- **Personality:** `friendly`
- **Ephemeral:** `true` (no disk persistence)

## Error Handling

- **Process dies:** `stream_response()` returns `Err`. Pipeline returns to listening. User restarts voice mode to recover.
- **Barge-in:** `cancel_turn()` sends `turn/interrupt`. Server preserves history up to interrupt. Next turn has full context.
- **Startup failure:** `start()` returns `Err`. Pipeline startup fails, UI shows error.
- **Multiple agentMessage items:** TTS all deltas regardless of phase (commentary + final_answer).

## Expected Latency Improvement

Eliminates process-spawn overhead (~0.5-1s per turn) and gains a persistent connection with token-level streaming. Combined with the subscription's server-side optimizations, expected reduction in time-to-first-audio of ~1-2s.
