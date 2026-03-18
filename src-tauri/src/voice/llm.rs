use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SYSTEM_PROMPT: &str = "You are in a live voice chat. Your replies are spoken aloud via TTS. Be concise — 1-2 sentences max. No markdown, no bullet points, no code blocks.";
const DEFAULT_MODEL: &str = "gpt-5.3-codex-spark";

// ---------------------------------------------------------------------------
// JSON-RPC helpers (codex omits "jsonrpc" field)
// ---------------------------------------------------------------------------

/// Build a JSON-RPC request (no `"jsonrpc"` field — codex omits it).
pub fn build_request(method: &str, id: u64, params: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "method": method,
        "id": id,
        "params": params,
    })
}

/// Build a JSON-RPC notification (no `id` field, no `"jsonrpc"` field).
pub fn build_notification(method: &str, params: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "method": method,
        "params": params,
    })
}

// ---------------------------------------------------------------------------
// CancelHandle — thread-safe handle for interrupting an in-progress turn
// ---------------------------------------------------------------------------

/// A thread-safe handle for cancelling an in-progress turn.
/// Only writes to stdin — does not read from stdout.
/// The LLM thread's stream_response() will see the resulting
/// turn/completed notification and exit naturally.
#[allow(dead_code)]
pub struct CancelHandle {
    writer: Arc<Mutex<BufWriter<ChildStdin>>>,
    thread_id: String,
    turn_id: String,
}

#[allow(dead_code)]
impl CancelHandle {
    /// Send turn/interrupt. Does not drain — stream_response sees
    /// turn/completed and exits naturally.
    pub fn send_interrupt(&self) -> Result<(), String> {
        let id = 0u64; // ID doesn't matter for fire-and-forget cancel
        let msg = build_request(
            "turn/interrupt",
            id,
            serde_json::json!({
                "threadId": self.thread_id,
                "turnId": self.turn_id,
            }),
        );
        let line =
            serde_json::to_string(&msg).map_err(|e| format!("Failed to serialize cancel: {e}"))?;
        let mut writer = self
            .writer
            .lock()
            .map_err(|e| format!("Failed to lock stdin for cancel: {e}"))?;
        writer
            .write_all(line.as_bytes())
            .map_err(|e| format!("Failed to write cancel: {e}"))?;
        writer
            .write_all(b"\n")
            .map_err(|e| format!("Failed to write cancel newline: {e}"))?;
        writer
            .flush()
            .map_err(|e| format!("Failed to flush cancel: {e}"))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// CodexAppServer
// ---------------------------------------------------------------------------

pub struct CodexAppServer {
    child: Child,
    writer: Arc<Mutex<BufWriter<ChildStdin>>>,
    reader: BufReader<ChildStdout>,
    thread_id: String,
    next_id: u64,
    current_turn_id: Option<String>,
}

impl CodexAppServer {
    /// Spawn `codex app-server` and perform the startup handshake.
    ///
    /// 1. Send `initialize` request
    /// 2. Send `initialized` notification
    /// 3. Send `thread/start` request → store `thread_id`
    /// 4. Drain the `thread/started` notification
    pub fn start(system_prompt: Option<&str>) -> Result<Self, String> {
        tracing::debug!("spawning codex app-server subprocess");
        let mut child = Command::new("codex")
            .arg("app-server")
            .env_remove("CLAUDECODE")
            .env_remove("CLAUDE_CODE_ENTRYPOINT")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                tracing::error!(error = %e, "failed to spawn codex app-server");
                format!("Failed to spawn codex app-server: {e}")
            })?;

        let stdin = child.stdin.take().ok_or("Failed to capture codex stdin")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Failed to capture codex stdout")?;

        let mut server = Self {
            child,
            writer: Arc::new(Mutex::new(BufWriter::new(stdin))),
            reader: BufReader::new(stdout),
            thread_id: String::new(),
            next_id: 0,
            current_turn_id: None,
        };

        tracing::debug!("codex app-server spawned, starting handshake");

        // 1. initialize
        let init_params = serde_json::json!({
            "clientInfo": {
                "name": "voice-pipeline",
                "title": "Voice Pipeline",
                "version": "0.1.0",
            },
            "capabilities": {
                "experimentalApi": true,
            },
        });
        server.send_request("initialize", init_params)?;

        // 2. initialized notification
        server.send_notification("initialized", serde_json::json!({}))?;

        // 3. thread/start
        let base_instructions = system_prompt.unwrap_or(SYSTEM_PROMPT);
        let thread_params = serde_json::json!({
            "model": DEFAULT_MODEL,
            "ephemeral": true,
            "approvalPolicy": "never",
            "sandbox": "danger-full-access",
            "baseInstructions": base_instructions,
            "personality": "friendly",
        });
        let thread_resp = server.send_request("thread/start", thread_params)?;

        let thread_id = thread_resp
            .get("result")
            .and_then(|r| r.get("thread"))
            .and_then(|t| t.get("id"))
            .and_then(|id| id.as_str())
            .ok_or("thread/start response missing result.thread.id")?
            .to_string();
        server.thread_id = thread_id;
        tracing::debug!("codex handshake complete, thread started");

        // 4. Drain notifications until thread/started
        loop {
            let msg = server.read_line()?;
            if msg.get("method").and_then(|m| m.as_str()) == Some("thread/started") {
                break;
            }
        }

        Ok(server)
    }

    /// Stream a response from the codex app-server.
    ///
    /// Sends `turn/start`, then reads notification lines until `turn/completed`.
    /// Calls `on_token` for each text delta. Returns the full accumulated response.
    pub fn stream_response(
        &mut self,
        text: &str,
        on_token: &mut dyn FnMut(&str),
    ) -> Result<String, String> {
        tracing::debug!("sending turn/start to codex");
        let turn_params = serde_json::json!({
            "threadId": self.thread_id,
            "input": [{"type": "text", "text": text}],
            "effort": "low",
        });
        let turn_resp = self.send_request("turn/start", turn_params)?;

        let turn_id = turn_resp
            .get("result")
            .and_then(|r| r.get("turn"))
            .and_then(|t| t.get("id"))
            .and_then(|id| id.as_str())
            .ok_or("turn/start response missing result.turn.id")?
            .to_string();
        self.current_turn_id = Some(turn_id);

        let mut full_response = String::new();

        loop {
            let msg = self.read_line()?;
            let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");

            match method {
                "item/agentMessage/delta" => {
                    if let Some(delta) = msg
                        .get("params")
                        .and_then(|p| p.get("delta"))
                        .and_then(|d| d.as_str())
                    {
                        full_response.push_str(delta);
                        on_token(delta);
                    }
                }
                "turn/completed" => {
                    self.current_turn_id = None;
                    let status = msg
                        .get("params")
                        .and_then(|p| p.get("status"))
                        .and_then(|s| s.as_str())
                        .unwrap_or("");
                    if status == "failed" {
                        let reason = msg
                            .get("params")
                            .and_then(|p| p.get("error"))
                            .and_then(|e| e.as_str())
                            .unwrap_or("unknown error");
                        tracing::error!(reason = %reason, "turn failed");
                        return Err(format!("Turn failed: {reason}"));
                    }
                    tracing::debug!("turn completed");
                    break;
                }
                "error" => {
                    let will_retry = msg
                        .get("params")
                        .and_then(|p| p.get("willRetry"))
                        .and_then(|w| w.as_bool())
                        .unwrap_or(false);
                    let error_msg = msg
                        .get("params")
                        .and_then(|p| p.get("message"))
                        .and_then(|m| m.as_str())
                        .unwrap_or("unknown error");

                    if !will_retry {
                        self.current_turn_id = None;
                        tracing::error!(error = %error_msg, "codex non-retriable error");
                        return Err(format!("Codex error (non-retriable): {error_msg}"));
                    }
                    tracing::warn!(error = %error_msg, "codex error, will retry");
                }
                _ => {
                    // Skip: turn/started, item/started, item/completed, reasoning/*, etc.
                }
            }
        }

        Ok(full_response)
    }

    /// Cancel the current turn (for barge-in).
    ///
    /// Sends `turn/interrupt` and drains until `turn/completed`.
    /// No-op if no turn is active.
    pub fn cancel_turn(&mut self) -> Result<(), String> {
        let turn_id = match self.current_turn_id.take() {
            Some(id) => id,
            None => return Ok(()),
        };

        tracing::debug!("sending turn/interrupt to codex");
        let interrupt_params = serde_json::json!({
            "threadId": self.thread_id,
            "turnId": turn_id,
        });
        self.send_request("turn/interrupt", interrupt_params)?;

        // Drain until turn/completed (also handle fatal errors to avoid hanging)
        loop {
            let msg = self.read_line()?;
            let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
            match method {
                "turn/completed" => break,
                "error" => {
                    let will_retry = msg["params"]["willRetry"].as_bool().unwrap_or(false);
                    if !will_retry {
                        let err_msg = msg["params"]["error"]["message"]
                            .as_str()
                            .unwrap_or("unknown");
                        return Err(format!("App-server error during cancel: {err_msg}"));
                    }
                }
                _ => {} // skip other notifications
            }
        }

        Ok(())
    }

    /// Create a cancel handle for the current turn. Returns None if no turn is active.
    #[allow(dead_code)]
    pub fn cancel_handle(&self) -> Option<CancelHandle> {
        self.current_turn_id.as_ref().map(|turn_id| CancelHandle {
            writer: self.writer.clone(),
            thread_id: self.thread_id.clone(),
            turn_id: turn_id.clone(),
        })
    }

    // -----------------------------------------------------------------------
    // Internal I/O helpers
    // -----------------------------------------------------------------------

    /// Serialize a message to JSON, write it as a single line + newline, and flush.
    fn write_message(&mut self, msg: &serde_json::Value) -> Result<(), String> {
        let line =
            serde_json::to_string(msg).map_err(|e| format!("Failed to serialize message: {e}"))?;
        let mut writer = self
            .writer
            .lock()
            .map_err(|e| format!("Failed to lock stdin: {e}"))?;
        writer
            .write_all(line.as_bytes())
            .map_err(|e| format!("Failed to write to codex stdin: {e}"))?;
        writer
            .write_all(b"\n")
            .map_err(|e| format!("Failed to write newline to codex stdin: {e}"))?;
        writer
            .flush()
            .map_err(|e| format!("Failed to flush codex stdin: {e}"))?;
        Ok(())
    }

    /// Read one line from stdout and parse it as JSON.
    fn read_line(&mut self) -> Result<serde_json::Value, String> {
        let mut line = String::new();
        let bytes_read = self
            .reader
            .read_line(&mut line)
            .map_err(|e| format!("Failed to read from codex stdout: {e}"))?;
        if bytes_read == 0 {
            return Err("codex app-server closed stdout (EOF)".to_string());
        }
        serde_json::from_str(&line)
            .map_err(|e| format!("Failed to parse codex output as JSON: {e}"))
    }

    /// Send a JSON-RPC request and read the matching response.
    fn send_request(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let msg = build_request(method, id, params);
        self.write_message(&msg)?;
        self.read_response(id)
    }

    /// Send a JSON-RPC notification (no response expected).
    fn send_notification(&mut self, method: &str, params: serde_json::Value) -> Result<(), String> {
        let msg = build_notification(method, params);
        self.write_message(&msg)
    }

    /// Read lines until we get a response with the expected `id`.
    /// Skips notifications (messages without an `id` field).
    /// Checks for an `error` field in the response.
    fn read_response(&mut self, expected_id: u64) -> Result<serde_json::Value, String> {
        loop {
            let msg = self.read_line()?;

            // Skip notifications (no id field)
            let msg_id = match msg.get("id") {
                Some(id) => id,
                None => continue,
            };

            let matches = match msg_id {
                serde_json::Value::Number(n) => n.as_u64() == Some(expected_id),
                _ => false,
            };

            if matches {
                // Check for error in response
                if let Some(err) = msg.get("error") {
                    let err_msg = err
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("unknown RPC error");
                    return Err(format!("JSON-RPC error: {err_msg}"));
                }
                return Ok(msg);
            }
        }
    }
}

impl Drop for CodexAppServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_request_has_method_id_params() {
        let req = build_request("initialize", 1, serde_json::json!({"foo": "bar"}));
        assert_eq!(req["method"], "initialize");
        assert_eq!(req["id"], 1);
        assert_eq!(req["params"]["foo"], "bar");
        // Must NOT have "jsonrpc" field
        assert!(req.get("jsonrpc").is_none());
    }

    #[test]
    fn build_notification_has_no_id() {
        let notif = build_notification("initialized", serde_json::json!({}));
        assert_eq!(notif["method"], "initialized");
        assert!(notif.get("id").is_none());
        assert!(notif.get("jsonrpc").is_none());
    }

    #[test]
    fn extract_thread_id_from_response() {
        let resp = serde_json::json!({
            "id": 2,
            "result": {
                "thread": {"id": "thread-abc-123"},
            },
        });
        let thread_id = resp
            .get("result")
            .and_then(|r| r.get("thread"))
            .and_then(|t| t.get("id"))
            .and_then(|id| id.as_str())
            .unwrap();
        assert_eq!(thread_id, "thread-abc-123");
    }

    #[test]
    fn extract_turn_id_from_response() {
        let resp = serde_json::json!({
            "id": 3,
            "result": {
                "turn": {"id": "turn-xyz-456"},
            },
        });
        let turn_id = resp
            .get("result")
            .and_then(|r| r.get("turn"))
            .and_then(|t| t.get("id"))
            .and_then(|id| id.as_str())
            .unwrap();
        assert_eq!(turn_id, "turn-xyz-456");
    }

    #[test]
    fn extract_delta_from_notification() {
        let notif = serde_json::json!({
            "method": "item/agentMessage/delta",
            "params": {
                "delta": "Hello ",
            },
        });
        let method = notif["method"].as_str().unwrap();
        assert_eq!(method, "item/agentMessage/delta");
        let delta = notif["params"]["delta"].as_str().unwrap();
        assert_eq!(delta, "Hello ");
    }

    #[test]
    fn detect_turn_completed() {
        let notif = serde_json::json!({
            "method": "turn/completed",
            "params": {
                "status": "completed",
            },
        });
        let method = notif["method"].as_str().unwrap();
        assert_eq!(method, "turn/completed");
        let status = notif["params"]["status"].as_str().unwrap();
        assert_eq!(status, "completed");
    }

    #[test]
    fn detect_turn_interrupted() {
        let notif = serde_json::json!({
            "method": "turn/completed",
            "params": {
                "status": "interrupted",
            },
        });
        let method = notif["method"].as_str().unwrap();
        assert_eq!(method, "turn/completed");
        let status = notif["params"]["status"].as_str().unwrap();
        assert_eq!(status, "interrupted");
    }

    #[test]
    fn initialize_message_is_well_formed() {
        let msg = build_request(
            "initialize",
            0,
            serde_json::json!({
                "clientInfo": {
                    "name": "voice-pipeline",
                    "title": "Voice Pipeline",
                    "version": "0.1.0",
                },
                "capabilities": {
                    "experimentalApi": true,
                },
            }),
        );
        assert_eq!(msg["method"], "initialize");
        assert_eq!(msg["id"], 0);
        assert_eq!(msg["params"]["clientInfo"]["name"], "voice-pipeline");
        assert_eq!(msg["params"]["clientInfo"]["title"], "Voice Pipeline");
        assert_eq!(msg["params"]["clientInfo"]["version"], "0.1.0");
        assert_eq!(msg["params"]["capabilities"]["experimentalApi"], true);
    }

    #[test]
    fn thread_start_message_is_well_formed() {
        let msg = build_request(
            "thread/start",
            1,
            serde_json::json!({
                "model": DEFAULT_MODEL,
                "ephemeral": true,
                "approvalPolicy": "never",
                "sandbox": "danger-full-access",
                "baseInstructions": SYSTEM_PROMPT,
                "personality": "friendly",
            }),
        );
        assert_eq!(msg["method"], "thread/start");
        assert_eq!(msg["params"]["model"], DEFAULT_MODEL);
        assert_eq!(msg["params"]["ephemeral"], true);
        assert_eq!(msg["params"]["approvalPolicy"], "never");
        assert_eq!(msg["params"]["sandbox"], "danger-full-access");
        assert_eq!(msg["params"]["baseInstructions"], SYSTEM_PROMPT);
        assert_eq!(msg["params"]["personality"], "friendly");
    }

    #[test]
    fn turn_start_message_is_well_formed() {
        let thread_id = "thread-abc-123";
        let msg = build_request(
            "turn/start",
            2,
            serde_json::json!({
                "threadId": thread_id,
                "input": [{"type": "text", "text": "hello"}],
                "effort": "low",
            }),
        );
        assert_eq!(msg["method"], "turn/start");
        assert_eq!(msg["params"]["threadId"], thread_id);
        assert_eq!(msg["params"]["input"][0]["type"], "text");
        assert_eq!(msg["params"]["input"][0]["text"], "hello");
        assert_eq!(msg["params"]["effort"], "low");
    }

    #[test]
    fn turn_interrupt_message_is_well_formed() {
        let msg = build_request(
            "turn/interrupt",
            5,
            serde_json::json!({
                "threadId": "thread-abc-123",
                "turnId": "turn-xyz-456",
            }),
        );
        assert_eq!(msg["method"], "turn/interrupt");
        assert_eq!(msg["params"]["threadId"], "thread-abc-123");
        assert_eq!(msg["params"]["turnId"], "turn-xyz-456");
    }
}
