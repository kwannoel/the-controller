use std::process::Stdio;
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;

const SYSTEM_PROMPT: &str = "You are a voice assistant. Keep responses concise and conversational. \
Speak naturally as if in a real-time voice conversation. Avoid markdown formatting, \
code blocks, or bullet points — respond as you would speak.";

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
pub async fn stream_response(
    conversation: &Conversation,
    on_token: &mut dyn FnMut(&str),
) -> Result<String, String> {
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
