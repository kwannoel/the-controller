use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

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
// CodexAppServer
// ---------------------------------------------------------------------------

pub struct CodexAppServer {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
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

        let stdin = child
            .stdin
            .take()
            .ok_or("Failed to capture codex stdin")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Failed to capture codex stdout")?;

        let mut server = Self {
            child,
            stdin: BufWriter::new(stdin),
            stdout: BufReader::new(stdout),
            thread_id: String::new(),
            next_id: 0,
            current_turn_id: None,
        };

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
        server.send_request("initialize", init_params).await?;

        // 2. initialized notification
        server
            .send_notification("initialized", serde_json::json!({}))
            .await?;

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
        let thread_resp = server.send_request("thread/start", thread_params).await?;

        let thread_id = thread_resp
            .get("result")
            .and_then(|r| r.get("threadId"))
            .and_then(|t| t.as_str())
            .ok_or("thread/start response missing threadId")?
            .to_string();
        server.thread_id = thread_id;

        // 4. Drain notifications until thread/started
        loop {
            let msg = server.read_line().await?;
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
    pub async fn stream_response(
        &mut self,
        text: &str,
        on_token: &mut dyn FnMut(&str),
    ) -> Result<String, String> {
        let turn_params = serde_json::json!({
            "threadId": self.thread_id,
            "input": [{"type": "text", "text": text}],
            "effort": "low",
        });
        let turn_resp = self.send_request("turn/start", turn_params).await?;

        let turn_id = turn_resp
            .get("result")
            .and_then(|r| r.get("turnId"))
            .and_then(|t| t.as_str())
            .ok_or("turn/start response missing turnId")?
            .to_string();
        self.current_turn_id = Some(turn_id);

        let mut full_response = String::new();

        loop {
            let msg = self.read_line().await?;
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
                        return Err(format!("Turn failed: {reason}"));
                    }
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
                        return Err(format!("Codex error (non-retriable): {error_msg}"));
                    }
                    eprintln!("[voice] Codex error (will retry): {error_msg}");
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
    pub async fn cancel_turn(&mut self) -> Result<(), String> {
        let turn_id = match self.current_turn_id.take() {
            Some(id) => id,
            None => return Ok(()),
        };

        let interrupt_params = serde_json::json!({
            "threadId": self.thread_id,
            "turnId": turn_id,
        });
        self.send_request("turn/interrupt", interrupt_params)
            .await?;

        // Drain until turn/completed (also handle fatal errors to avoid hanging)
        loop {
            let msg = self.read_line().await?;
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

    // -----------------------------------------------------------------------
    // Internal I/O helpers
    // -----------------------------------------------------------------------

    /// Send a JSON-RPC request and read the matching response.
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

    /// Send a JSON-RPC notification (no response expected).
    async fn send_notification(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<(), String> {
        let msg = build_notification(method, params);
        self.write_message(&msg).await
    }

    /// Serialize a message to JSON, write it as a single line + newline, and flush.
    async fn write_message(&mut self, msg: &serde_json::Value) -> Result<(), String> {
        let line = serde_json::to_string(msg)
            .map_err(|e| format!("Failed to serialize message: {e}"))?;
        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| format!("Failed to write to codex stdin: {e}"))?;
        self.stdin
            .write_all(b"\n")
            .await
            .map_err(|e| format!("Failed to write newline to codex stdin: {e}"))?;
        self.stdin
            .flush()
            .await
            .map_err(|e| format!("Failed to flush codex stdin: {e}"))?;
        Ok(())
    }

    /// Read one line from stdout and parse it as JSON.
    async fn read_line(&mut self) -> Result<serde_json::Value, String> {
        let mut line = String::new();
        let bytes_read = self
            .stdout
            .read_line(&mut line)
            .await
            .map_err(|e| format!("Failed to read from codex stdout: {e}"))?;
        if bytes_read == 0 {
            return Err("codex app-server closed stdout (EOF)".to_string());
        }
        serde_json::from_str(&line)
            .map_err(|e| format!("Failed to parse codex output as JSON: {e}"))
    }

    /// Read lines until we get a response with the expected `id`.
    /// Skips notifications (messages without an `id` field).
    /// Checks for an `error` field in the response.
    async fn read_response(&mut self, expected_id: u64) -> Result<serde_json::Value, String> {
        loop {
            let msg = self.read_line().await?;

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
        let _ = self.child.start_kill();
    }
}

// ---------------------------------------------------------------------------
// Legacy code — still used by mod.rs, will be removed in a later task
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn live_chat_dir() -> Result<std::path::PathBuf, String> {
    let dir = dirs::home_dir()
        .ok_or("Cannot determine home directory")?
        .join(".the-controller")
        .join("live-chat");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create live-chat dir: {e}"))?;
    Ok(dir)
}

#[derive(Clone)]
pub struct Conversation {
    pub messages: Vec<(String, String)>, // (role, content)
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
#[allow(dead_code)]
pub async fn stream_response(
    conversation: &Conversation,
    on_token: &mut dyn FnMut(&str),
) -> Result<String, String> {
    let prompt = conversation
        .messages
        .last()
        .map(|(_, content)| content.as_str())
        .unwrap_or("");

    let is_first_turn = conversation.messages.len() <= 1;

    let mut cmd = Command::new("claude");
    cmd.arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
        .arg("--model")
        .arg("haiku");

    if is_first_turn {
        cmd.arg("--system-prompt")
            .arg(conversation.system_prompt());
    } else {
        cmd.arg("--continue");
    }

    cmd.arg("-p")
        .arg(prompt)
        .current_dir(live_chat_dir()?)
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

    let mut reader = tokio::io::BufReader::new(stdout).lines();
    let mut full_response = String::new();

    while let Some(line) = reader
        .next_line()
        .await
        .map_err(|e| format!("Failed to read claude output: {e}"))?
    {
        if line.is_empty() {
            continue;
        }

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Legacy Conversation tests (kept) --

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
        assert!(conv.system_prompt().contains("voice chat"));
    }

    // -- JSON-RPC helper tests --

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
                "threadId": "thread-abc-123",
            },
        });
        let thread_id = resp
            .get("result")
            .and_then(|r| r.get("threadId"))
            .and_then(|t| t.as_str())
            .unwrap();
        assert_eq!(thread_id, "thread-abc-123");
    }

    #[test]
    fn extract_turn_id_from_response() {
        let resp = serde_json::json!({
            "id": 3,
            "result": {
                "turnId": "turn-xyz-456",
            },
        });
        let turn_id = resp
            .get("result")
            .and_then(|r| r.get("turnId"))
            .and_then(|t| t.as_str())
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
