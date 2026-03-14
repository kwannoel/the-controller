# Voice Codex App-Server Integration — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Replace the `claude` CLI LLM backend in voice mode with a persistent `codex app-server` process for lower latency token streaming.

**Architecture:** Rewrite `llm.rs` to manage a long-lived `codex app-server` child process communicating via JSON-RPC 2.0 over stdio. Update `mod.rs` to use `CodexAppServer` instead of `Conversation` for LLM calls. Keep all other voice pipeline components unchanged.

**Tech Stack:** Rust, Tauri v2, tokio (async I/O), serde_json (JSON-RPC), crossbeam-channel (sentence streaming)

**Design doc:** `docs/plans/2026-03-14-voice-codex-appserver-design.md`

---

### Task 1: Write CodexAppServer JSON-RPC helpers and struct

**Files:**
- Rewrite: `src-tauri/src/voice/llm.rs`

**Step 1: Write tests for JSON-RPC message building**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_request_has_method_id_params() {
        let msg = build_request("initialize", 0, serde_json::json!({"foo": "bar"}));
        assert_eq!(msg["method"], "initialize");
        assert_eq!(msg["id"], 0);
        assert_eq!(msg["params"]["foo"], "bar");
        assert!(msg.get("jsonrpc").is_none()); // codex omits jsonrpc field
    }

    #[test]
    fn build_notification_has_no_id() {
        let msg = build_notification("initialized", serde_json::json!({}));
        assert_eq!(msg["method"], "initialized");
        assert!(msg.get("id").is_none());
    }

    #[test]
    fn extract_thread_id_from_response() {
        let resp = serde_json::json!({
            "id": 1,
            "result": {
                "thread": {"id": "thr_abc123"},
                "model": "gpt-5.3-codex-spark"
            }
        });
        let tid = resp["result"]["thread"]["id"].as_str().unwrap();
        assert_eq!(tid, "thr_abc123");
    }

    #[test]
    fn extract_turn_id_from_response() {
        let resp = serde_json::json!({
            "id": 2,
            "result": {
                "turn": {"id": "turn_xyz789", "status": "inProgress", "items": []}
            }
        });
        let tid = resp["result"]["turn"]["id"].as_str().unwrap();
        assert_eq!(tid, "turn_xyz789");
    }

    #[test]
    fn extract_delta_from_notification() {
        let notif = serde_json::json!({
            "method": "item/agentMessage/delta",
            "params": {
                "threadId": "thr_1",
                "turnId": "turn_1",
                "itemId": "item_1",
                "delta": "Hello world"
            }
        });
        let delta = notif["params"]["delta"].as_str().unwrap();
        assert_eq!(delta, "Hello world");
    }

    #[test]
    fn detect_turn_completed() {
        let notif = serde_json::json!({
            "method": "turn/completed",
            "params": {
                "threadId": "thr_1",
                "turn": {"id": "turn_1", "status": "completed", "items": [], "error": null}
            }
        });
        assert_eq!(notif["method"], "turn/completed");
        assert_eq!(notif["params"]["turn"]["status"], "completed");
    }

    #[test]
    fn detect_turn_interrupted() {
        let notif = serde_json::json!({
            "method": "turn/completed",
            "params": {
                "threadId": "thr_1",
                "turn": {"id": "turn_1", "status": "interrupted", "items": [], "error": null}
            }
        });
        assert_eq!(notif["params"]["turn"]["status"], "interrupted");
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test voice::llm::tests --lib 2>&1 | tail -20`
Expected: FAIL — `build_request`, `build_notification` not defined

**Step 3: Write the helper functions and CodexAppServer struct**

Replace the entire contents of `llm.rs` with:

```rust
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

const SYSTEM_PROMPT: &str = "You are in a live voice chat. Your replies are spoken aloud via TTS. Be concise — 1-2 sentences max. No markdown, no bullet points, no code blocks.";
const DEFAULT_MODEL: &str = "gpt-5.3-codex-spark";

pub struct CodexAppServer {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    thread_id: String,
    next_id: u64,
    current_turn_id: Option<String>,
}

fn build_request(method: &str, id: u64, params: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "method": method,
        "id": id,
        "params": params,
    })
}

fn build_notification(method: &str, params: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "method": method,
        "params": params,
    })
}
```

**Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test voice::llm::tests --lib 2>&1 | tail -20`
Expected: All 7 tests PASS

**Step 5: Commit**

```bash
git add src-tauri/src/voice/llm.rs
git commit -m "refactor(voice): add CodexAppServer struct and JSON-RPC helpers"
```

---

### Task 2: Implement CodexAppServer::start()

**Files:**
- Modify: `src-tauri/src/voice/llm.rs`

**Step 1: Write test for initialization message sequence**

Add to tests module:

```rust
#[test]
fn initialize_message_is_well_formed() {
    let msg = build_request("initialize", 0, serde_json::json!({
        "clientInfo": {
            "name": "voice-pipeline",
            "title": "Voice Pipeline",
            "version": "0.1.0"
        },
        "capabilities": {
            "experimentalApi": true
        }
    }));
    assert_eq!(msg["method"], "initialize");
    assert_eq!(msg["params"]["clientInfo"]["name"], "voice-pipeline");
    assert!(msg["params"]["capabilities"]["experimentalApi"].as_bool().unwrap());
}

#[test]
fn thread_start_message_is_well_formed() {
    let msg = build_request("thread/start", 1, serde_json::json!({
        "model": DEFAULT_MODEL,
        "ephemeral": true,
        "approvalPolicy": "never",
        "sandbox": "danger-full-access",
        "baseInstructions": SYSTEM_PROMPT,
        "personality": "friendly",
    }));
    assert_eq!(msg["params"]["model"], DEFAULT_MODEL);
    assert_eq!(msg["params"]["ephemeral"], true);
    assert_eq!(msg["params"]["approvalPolicy"], "never");
    assert_eq!(msg["params"]["baseInstructions"], SYSTEM_PROMPT);
}
```

**Step 2: Run tests to verify they pass** (these test message structure only)

Run: `cd src-tauri && cargo test voice::llm::tests --lib 2>&1 | tail -20`
Expected: PASS

**Step 3: Implement start(), send_request(), send_notification(), read_response()**

Add to `llm.rs`:

```rust
impl CodexAppServer {
    /// Spawn codex app-server, perform handshake, and create a thread.
    pub async fn start(system_prompt: Option<&str>) -> Result<Self, String> {
        let mut child = Command::new("codex")
            .arg("app-server")
            .env_remove("CLAUDECODE")
            .env_remove("CLAUDE_CODE_ENTRYPOINT")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn codex app-server: {e}"))?;

        let stdin = BufWriter::new(
            child.stdin.take().ok_or("Failed to capture app-server stdin")?,
        );
        let stdout = BufReader::new(
            child.stdout.take().ok_or("Failed to capture app-server stdout")?,
        );

        let mut server = Self {
            child,
            stdin,
            stdout,
            thread_id: String::new(),
            next_id: 0,
            current_turn_id: None,
        };

        // Step 1: initialize
        let init_resp = server
            .send_request(
                "initialize",
                serde_json::json!({
                    "clientInfo": {
                        "name": "voice-pipeline",
                        "title": "Voice Pipeline",
                        "version": "0.1.0"
                    },
                    "capabilities": {
                        "experimentalApi": true
                    }
                }),
            )
            .await?;

        eprintln!(
            "[voice] codex app-server: {}",
            init_resp["result"]["userAgent"].as_str().unwrap_or("unknown")
        );

        // Step 2: initialized notification
        server
            .send_notification("initialized", serde_json::json!({}))
            .await?;

        // Step 3: thread/start
        let prompt = system_prompt.unwrap_or(SYSTEM_PROMPT);
        let thread_resp = server
            .send_request(
                "thread/start",
                serde_json::json!({
                    "model": DEFAULT_MODEL,
                    "ephemeral": true,
                    "approvalPolicy": "never",
                    "sandbox": "danger-full-access",
                    "baseInstructions": prompt,
                    "personality": "friendly",
                }),
            )
            .await?;

        server.thread_id = thread_resp["result"]["thread"]["id"]
            .as_str()
            .ok_or("thread/start response missing thread.id")?
            .to_string();

        // Drain the thread/started notification
        server.read_line().await?;

        eprintln!("[voice] codex thread: {}", server.thread_id);

        Ok(server)
    }

    async fn send_request(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let msg = build_request(method, id, params);
        self.write_message(&msg).await?;
        self.read_response(id).await
    }

    async fn send_notification(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<(), String> {
        let msg = build_notification(method, params);
        self.write_message(&msg).await
    }

    async fn write_message(&mut self, msg: &serde_json::Value) -> Result<(), String> {
        let line = serde_json::to_string(msg)
            .map_err(|e| format!("Failed to serialize message: {e}"))?;
        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| format!("Failed to write to app-server: {e}"))?;
        self.stdin
            .write_all(b"\n")
            .await
            .map_err(|e| format!("Failed to write newline: {e}"))?;
        self.stdin
            .flush()
            .await
            .map_err(|e| format!("Failed to flush to app-server: {e}"))?;
        Ok(())
    }

    async fn read_line(&mut self) -> Result<serde_json::Value, String> {
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .await
            .map_err(|e| format!("Failed to read from app-server: {e}"))?;
        if line.is_empty() {
            return Err("App-server closed stdout".to_string());
        }
        serde_json::from_str(&line)
            .map_err(|e| format!("Failed to parse app-server message: {e}: {line}"))
    }

    /// Read lines until we get a response matching the given request id.
    /// Non-response messages (notifications) are skipped.
    async fn read_response(&mut self, expected_id: u64) -> Result<serde_json::Value, String> {
        loop {
            let msg = self.read_line().await?;
            // Responses have "id" field, notifications have "method" field
            if let Some(id) = msg.get("id") {
                let msg_id = id.as_u64().unwrap_or(u64::MAX);
                if msg_id == expected_id {
                    if let Some(error) = msg.get("error") {
                        return Err(format!(
                            "App-server error: {}",
                            error["message"].as_str().unwrap_or("unknown")
                        ));
                    }
                    return Ok(msg);
                }
            }
            // Skip notifications during handshake
        }
    }
}

impl Drop for CodexAppServer {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}
```

**Step 4: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | tail -10`
Expected: No errors (warnings OK)

**Step 5: Commit**

```bash
git add src-tauri/src/voice/llm.rs
git commit -m "feat(voice): implement CodexAppServer::start() with handshake"
```

---

### Task 3: Implement stream_response()

**Files:**
- Modify: `src-tauri/src/voice/llm.rs`

**Step 1: Write test for turn/start message construction**

Add to tests:

```rust
#[test]
fn turn_start_message_is_well_formed() {
    let thread_id = "thr_abc123";
    let user_text = "What is the weather?";
    let msg = build_request("turn/start", 5, serde_json::json!({
        "threadId": thread_id,
        "input": [{"type": "text", "text": user_text}],
        "effort": "low",
    }));
    assert_eq!(msg["params"]["threadId"], thread_id);
    assert_eq!(msg["params"]["input"][0]["type"], "text");
    assert_eq!(msg["params"]["input"][0]["text"], user_text);
    assert_eq!(msg["params"]["effort"], "low");
}
```

**Step 2: Run test**

Run: `cd src-tauri && cargo test voice::llm::tests::turn_start --lib 2>&1 | tail -10`
Expected: PASS

**Step 3: Implement stream_response()**

Add to `impl CodexAppServer`:

```rust
    /// Send user text and stream response tokens via callback.
    /// Returns the full accumulated response text.
    pub async fn stream_response(
        &mut self,
        text: &str,
        on_token: &mut dyn FnMut(&str),
    ) -> Result<String, String> {
        let turn_resp = self
            .send_request(
                "turn/start",
                serde_json::json!({
                    "threadId": self.thread_id,
                    "input": [{"type": "text", "text": text}],
                    "effort": "low",
                }),
            )
            .await?;

        self.current_turn_id = turn_resp["result"]["turn"]["id"]
            .as_str()
            .map(|s| s.to_string());

        let mut full_response = String::new();

        loop {
            let msg = self.read_line().await?;
            let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");

            match method {
                "item/agentMessage/delta" => {
                    if let Some(delta) = msg["params"]["delta"].as_str() {
                        full_response.push_str(delta);
                        on_token(delta);
                    }
                }
                "turn/completed" => {
                    self.current_turn_id = None;
                    let status = msg["params"]["turn"]["status"]
                        .as_str()
                        .unwrap_or("unknown");
                    if status == "failed" {
                        let err_msg = msg["params"]["turn"]["error"]["message"]
                            .as_str()
                            .unwrap_or("unknown error");
                        return Err(format!("Turn failed: {err_msg}"));
                    }
                    break;
                }
                "error" => {
                    let will_retry = msg["params"]["willRetry"].as_bool().unwrap_or(false);
                    let err_msg = msg["params"]["error"]["message"]
                        .as_str()
                        .unwrap_or("unknown");
                    if !will_retry {
                        return Err(format!("App-server error: {err_msg}"));
                    }
                    eprintln!("[voice] app-server retrying: {err_msg}");
                }
                _ => {
                    // Skip: turn/started, item/started, item/completed,
                    // item/reasoning/*, etc.
                }
            }
        }

        Ok(full_response)
    }
```

**Step 4: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | tail -10`
Expected: No errors

**Step 5: Commit**

```bash
git add src-tauri/src/voice/llm.rs
git commit -m "feat(voice): implement CodexAppServer::stream_response()"
```

---

### Task 4: Implement cancel_turn()

**Files:**
- Modify: `src-tauri/src/voice/llm.rs`

**Step 1: Write test for interrupt message construction**

Add to tests:

```rust
#[test]
fn turn_interrupt_message_is_well_formed() {
    let msg = build_request("turn/interrupt", 10, serde_json::json!({
        "threadId": "thr_abc",
        "turnId": "turn_xyz",
    }));
    assert_eq!(msg["method"], "turn/interrupt");
    assert_eq!(msg["params"]["threadId"], "thr_abc");
    assert_eq!(msg["params"]["turnId"], "turn_xyz");
}
```

**Step 2: Run test**

Run: `cd src-tauri && cargo test voice::llm::tests::turn_interrupt --lib 2>&1 | tail -10`
Expected: PASS

**Step 3: Implement cancel_turn()**

Add to `impl CodexAppServer`:

```rust
    /// Cancel the current turn (barge-in). No-op if no turn is active.
    pub async fn cancel_turn(&mut self) -> Result<(), String> {
        let turn_id = match self.current_turn_id.take() {
            Some(id) => id,
            None => return Ok(()),
        };

        self.send_request(
            "turn/interrupt",
            serde_json::json!({
                "threadId": self.thread_id,
                "turnId": turn_id,
            }),
        )
        .await?;

        // Drain until turn/completed
        loop {
            let msg = self.read_line().await?;
            let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
            if method == "turn/completed" {
                break;
            }
        }

        Ok(())
    }
```

**Step 4: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | tail -10`
Expected: No errors

**Step 5: Commit**

```bash
git add src-tauri/src/voice/llm.rs
git commit -m "feat(voice): implement CodexAppServer::cancel_turn() for barge-in"
```

---

### Task 5: Update mod.rs to use CodexAppServer

**Files:**
- Modify: `src-tauri/src/voice/mod.rs`

**Step 1: Update SpeechContext to hold CodexAppServer**

Replace the `conversation` field in `SpeechContext`:

```rust
struct SpeechContext<'a> {
    whisper: &'a stt::WhisperStt,
    tts_engine: &'a mut tts::PiperTts,
    audio_out: &'a audio_output::AudioOutput,
    app_server: &'a mut llm::CodexAppServer,
    vad_engine: &'a mut vad::Vad,
    auto_gain: &'a mut gain::AutoGain,
    audio_rx: &'a Receiver<Vec<i16>>,
    emitter: &'a Arc<dyn EventEmitter>,
    stop: &'a Arc<AtomicBool>,
}
```

**Step 2: Update run_pipeline() to spawn CodexAppServer**

In `run_pipeline()`, replace `let mut conversation = llm::Conversation::new(None);` with:

```rust
    // Start codex app-server (needs a tokio runtime for async I/O)
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("Failed to create tokio runtime: {e}"))?;
    let mut app_server = rt.block_on(llm::CodexAppServer::start(None))
        .map_err(|e| format!("Failed to start codex app-server: {e}"))?;
```

And update the `SpeechContext` construction to use `app_server: &mut app_server` instead of `conversation: &mut conversation`.

**Step 3: Update process_speech() — remove conversation.add_user(), change LLM thread**

In `process_speech()`, the LLM thread section (lines 274-328) changes to:

```rust
    eprintln!("[voice] You: {text}");
    emit_debug(ctx.emitter, &format!("stt: \"{}\"", text));
    let _ = ctx.emitter.emit(
        "voice-transcript",
        &serde_json::json!({"role": "user", "text": text}).to_string(),
    );

    // Stream LLM → TTS → Audio concurrently.
    emit_debug(ctx.emitter, "llm: streaming...");

    // We need to pass the app_server into the LLM thread. Since CodexAppServer
    // uses async I/O, the thread creates its own tokio runtime.
    // We use a crossbeam channel to send the text and receive the result,
    // while the app_server stays on the main thread inside a tokio block_on.
    //
    // Actually: the LLM must run concurrently with TTS. We send tokens via
    // sentence_tx from the LLM callback, same pattern as before.
    // The difference: we call app_server.stream_response() which is async,
    // so we need a tokio runtime in the LLM thread.

    let emitter_for_llm = ctx.emitter.clone();
    let (sentence_tx, sentence_rx) = crossbeam_channel::bounded::<String>(8);

    // We can't move app_server into the thread (it's borrowed).
    // Instead, we run stream_response on the current thread inside a
    // tokio runtime, and spawn the TTS consumer on a separate thread.
    // But that inverts the current pattern...
    //
    // Simpler: take a mutable reference via a channel-based approach.
    // The LLM thread needs owned access to app_server's stdin/stdout.
    //
    // Cleanest approach: run stream_response in a background thread with
    // its own tokio runtime, and pass app_server by &mut via unsafe or
    // by temporarily moving it.
```

Actually — the cleanest approach given the existing architecture is to **temporarily move** `app_server` into the LLM thread and get it back via the join handle.

Update `process_speech` signature to take `app_server` by value, return it back:

Change `process_speech` to:

```rust
fn process_speech(
    audio: &[f32],
    ctx: &mut SpeechContext<'_>,
    app_server: &mut llm::CodexAppServer,
) -> Result<SpeechResult, String> {
```

Remove `app_server` from `SpeechContext`. The LLM thread section becomes:

```rust
    let emitter_for_llm = ctx.emitter.clone();
    let (sentence_tx, sentence_rx) = crossbeam_channel::bounded::<String>(8);
    let user_text = text.clone();

    // Temporarily take ownership via Option swap for the thread
    // Actually we can't move a &mut. Use a different pattern:
    // Create a channel pair to bridge sync/async boundary.
    let (token_tx, token_rx) = crossbeam_channel::bounded::<Result<String, String>>(64);

    // Spawn LLM thread that owns a new tokio runtime
    // and communicates tokens back via channel
    let (req_tx, req_rx) = std::sync::mpsc::channel::<(String, crossbeam_channel::Sender<Result<String, String>>)>();
```

Wait — this is getting complex. Let me reconsider the threading model.

**Revised approach:** Keep the LLM call on the voice pipeline thread inside a tokio runtime. Run sentence TTS consumption in the `crossbeam::select!` loop as before. The key insight: `stream_response()` calls `on_token()` synchronously for each delta. The `on_token` callback sends sentences via `sentence_tx`. The main thread doesn't need to be doing something else during `stream_response()` — that's what the sentence channel is for.

The existing pattern already handles this correctly:
1. LLM thread: runs `stream_response()`, callback sends sentences via `sentence_tx`
2. Main thread: `crossbeam::select!` on `sentence_rx` + `audio_rx` for TTS + barge-in

The only challenge is that `app_server` (with its stdin/stdout handles) can't be shared between threads easily. Solution: **use a `std::sync::Mutex` wrapper** so the LLM thread can lock it.

Actually even simpler: since only one thread uses app_server at a time (the LLM thread during streaming, the main thread for cancel_turn during barge-in), we can wrap it in `Arc<tokio::sync::Mutex>`. But mixing tokio mutex with crossbeam gets messy.

**Final clean approach:** Use `unsafe` to send `&mut app_server` to the LLM thread. This is safe because:
- Only the LLM thread calls `stream_response()` during a turn
- The main thread only calls `cancel_turn()` after the LLM thread has been abandoned (barge-in breaks out of the select loop, then cancel_turn runs after)
- These never overlap on the same `&mut`

Wrap in a `SendPtr` newtype:

```rust
struct SendPtr<T>(*mut T);
unsafe impl<T> Send for SendPtr<T> {}
```

This is the pattern used elsewhere in the codebase for similar cross-thread mutable access.

Here's the final `process_speech` LLM section:

```rust
    emit_debug(ctx.emitter, "llm: streaming...");

    let emitter_for_llm = ctx.emitter.clone();
    let (sentence_tx, sentence_rx) = crossbeam_channel::bounded::<String>(8);
    let user_text = text.clone();

    // SAFETY: app_server is only accessed by one thread at a time.
    // The LLM thread uses it for stream_response().
    // The main thread uses it for cancel_turn() only after the LLM thread
    // is no longer reading (barge-in breaks out of select, then we cancel).
    let app_server_ptr = SendPtr(app_server as *mut llm::CodexAppServer);

    let llm_handle = std::thread::spawn(move || -> Result<String, String> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create tokio runtime: {e}"))?;
        rt.block_on(async {
            let server = unsafe { &mut *app_server_ptr.0 };
            let mut sentence_buf = String::new();
            let mut full_response = String::new();
            server
                .stream_response(&user_text, &mut |token| {
                    sentence_buf.push_str(token);
                    full_response.push_str(token);
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
            Ok(full_response)
        })
    });
```

**Step 4: Update barge-in handling**

After barge-in is confirmed and `playback.cancel()` is called, cancel the LLM turn:

```rust
    if interrupted {
        playback.cancel();
        // Cancel the server-side turn so it stops generating
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create runtime for cancel: {e}"))?;
        if let Err(e) = rt.block_on(app_server.cancel_turn()) {
            eprintln!("[voice] Failed to cancel turn: {e}");
        }
    }
```

Replace the old `ctx.conversation.add_assistant(...)` calls:
- Normal completion: just join `llm_handle`, response is already in server-side history
- Interrupted: emit transcript with `[interrupted]` suffix (same as before), server preserves partial history

```rust
    if interrupted {
        let partial = spoken_sentences.join(" ");
        if !partial.is_empty() {
            eprintln!("[voice] Assistant (interrupted): {partial}");
            let _ = ctx.emitter.emit(
                "voice-transcript",
                &serde_json::json!({"role": "assistant", "text": format!("{partial}…")})
                    .to_string(),
            );
        }
    } else {
        match llm_handle.join() {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err("LLM thread panicked".to_string()),
        }
    }
```

**Step 5: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | tail -20`
Expected: No errors

**Step 6: Commit**

```bash
git add src-tauri/src/voice/mod.rs src-tauri/src/voice/llm.rs
git commit -m "feat(voice): wire CodexAppServer into voice pipeline"
```

---

### Task 6: Remove dead code and update tests

**Files:**
- Modify: `src-tauri/src/voice/llm.rs`

**Step 1: Remove old Conversation struct and tests**

Delete `Conversation`, `live_chat_dir()`, old `stream_response()`, and the old tests (`conversation_tracks_messages`, `conversation_uses_custom_persona`, `conversation_uses_default_system_prompt`).

**Step 2: Verify all tests pass**

Run: `cd src-tauri && cargo test voice:: --lib 2>&1 | tail -20`
Expected: All voice module tests pass (llm, tts, mod tests)

**Step 3: Verify full project compiles**

Run: `cd src-tauri && cargo check 2>&1 | tail -10`
Expected: No errors

**Step 4: Commit**

```bash
git add src-tauri/src/voice/llm.rs
git commit -m "refactor(voice): remove dead claude CLI code and Conversation struct"
```

---

### Task 7: Integration smoke test

**Files:** None (manual testing)

**Step 1: Run the app in dev mode**

Run: `npm run tauri dev`

**Step 2: Test voice mode**

1. Open voice mode (hotkey or UI)
2. Verify "Listening" state appears (codex app-server started successfully)
3. Speak a short phrase ("What's two plus two?")
4. Verify response is spoken back
5. Verify debug panel shows `codex app-server` startup log and `item/agentMessage/delta` events

**Step 3: Test barge-in**

1. Ask a question that produces a long response ("Tell me about the history of computers")
2. While assistant is speaking, say something
3. Verify assistant stops, processes your new input

**Step 4: Test multi-turn**

1. Ask "What's the capital of France?"
2. Wait for response
3. Ask "And what about Germany?"
4. Verify the response shows awareness of the previous turn (conversation continuity)

**Step 5: Commit if any fixes were needed**

```bash
git add -A
git commit -m "fix(voice): address issues found during integration testing"
```

---

### Task 8: Run all existing tests

**Files:** None

**Step 1: Run Rust tests**

Run: `cd src-tauri && cargo test 2>&1 | tail -30`
Expected: All tests pass

**Step 2: Run frontend tests**

Run: `npx vitest run 2>&1 | tail -30`
Expected: All tests pass

**Step 3: Final commit if needed**

If any tests needed fixing, commit the fixes.
