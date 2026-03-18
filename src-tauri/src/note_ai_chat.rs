use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteAiChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NoteAiResponse {
    Replace { text: String },
    Info { text: String },
}

fn build_note_ai_prompt(
    note_content: &str,
    selected_text: &str,
    conversation_history: &[NoteAiChatMessage],
    prompt: &str,
) -> String {
    let mut parts = Vec::new();

    parts.push(
        "You are a note-editing AI assistant. The user has selected text in a note and is asking you to help with it.\n\
        \n\
        You MUST return ONLY valid JSON with one of these shapes:\n\
        {\"type\":\"replace\",\"text\":\"the new text that will replace the selection\"}\n\
        {\"type\":\"info\",\"text\":\"an informational response that does not modify the note\"}\n\
        \n\
        Use \"replace\" when the user wants to modify, rewrite, fix, or transform the selected text.\n\
        Use \"info\" when the user is asking a question about the text or wants an explanation without changes.\n\
        \n\
        The note supports markdown image syntax: ![description](url) for images.\n\
        You can include images using URLs when relevant to the user's request.\n\
        \n\
        If the user asks you to revert, return a \"replace\" with the original selected text.\n\
        \n\
        Do NOT wrap JSON in markdown code fences. Return raw JSON only.".to_string(),
    );

    parts.push(format!(
        "--- NOTE CONTENT ---\n{}\n--- END NOTE CONTENT ---",
        note_content
    ));
    parts.push(format!(
        "--- SELECTED TEXT ---\n{}\n--- END SELECTED TEXT ---",
        selected_text
    ));

    if !conversation_history.is_empty() {
        let history_json =
            serde_json::to_string(conversation_history).unwrap_or_else(|_| "[]".to_string());
        parts.push(format!(
            "--- CONVERSATION HISTORY ---\n{}\n--- END CONVERSATION HISTORY ---",
            history_json
        ));
    }

    parts.push(format!("User prompt: {}", prompt));

    parts.join("\n\n")
}

pub fn parse_note_ai_response(raw: &str) -> Result<NoteAiResponse, String> {
    let result =
        serde_json::from_str(raw).map_err(|e| format!("Failed to parse note AI response: {}", e));
    if result.is_err() {
        tracing::error!("failed to parse note AI response as JSON");
    }
    result
}

fn run_note_ai_turn(repo_path: String, prompt: String) -> Result<NoteAiResponse, String> {
    tracing::debug!(repo_path, "invoking codex exec for note AI");
    let output = std::process::Command::new("codex")
        .arg("exec")
        .arg("--skip-git-repo-check")
        .arg("--sandbox")
        .arg("danger-full-access")
        .arg(&prompt)
        .current_dir(&repo_path)
        .env_remove("CLAUDECODE")
        .output()
        .map_err(|e| {
            tracing::error!(error = %e, "failed to spawn codex exec");
            format!("Failed to run codex exec: {}", e)
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(status = %output.status, "codex exec returned non-zero exit status");
        return Err(format!("codex exec failed: {}", stderr.trim()));
    }

    tracing::debug!("codex exec completed, parsing response");
    parse_note_ai_response(String::from_utf8_lossy(&output.stdout).trim())
}

pub async fn send_note_ai_message(
    repo_path: String,
    note_content: String,
    selected_text: String,
    conversation_history: Vec<NoteAiChatMessage>,
    prompt: String,
) -> Result<NoteAiResponse, String> {
    tracing::debug!(
        repo_path,
        history_len = conversation_history.len(),
        "starting note AI chat turn"
    );
    let full_prompt = build_note_ai_prompt(
        &note_content,
        &selected_text,
        &conversation_history,
        &prompt,
    );

    let result = tokio::task::spawn_blocking(move || run_note_ai_turn(repo_path, full_prompt))
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "note AI blocking task panicked");
            format!("Task failed: {}", e)
        })?;

    match &result {
        Ok(_) => tracing::debug!("note AI chat turn completed"),
        Err(e) => tracing::error!(error = %e, "note AI chat turn failed"),
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_replace_response() {
        let raw = r#"{"type":"replace","text":"new content"}"#;
        let response = parse_note_ai_response(raw).unwrap();
        assert_eq!(
            response,
            NoteAiResponse::Replace {
                text: "new content".to_string()
            }
        );
    }

    #[test]
    fn parse_info_response() {
        let raw = r#"{"type":"info","text":"explanation"}"#;
        let response = parse_note_ai_response(raw).unwrap();
        assert_eq!(
            response,
            NoteAiResponse::Info {
                text: "explanation".to_string()
            }
        );
    }

    #[test]
    fn parse_invalid_response() {
        let raw = "this is not json";
        let result = parse_note_ai_response(raw);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to parse"));
    }

    #[test]
    fn build_prompt_includes_note_and_selection() {
        let prompt = build_note_ai_prompt(
            "# My Note\nSome content here.",
            "Some content",
            &[],
            "Make this bold",
        );

        assert!(prompt.contains("# My Note\nSome content here."));
        assert!(prompt.contains("Some content"));
        assert!(prompt.contains("Make this bold"));
        assert!(prompt.contains("--- NOTE CONTENT ---"));
        assert!(prompt.contains("--- END NOTE CONTENT ---"));
        assert!(prompt.contains("--- SELECTED TEXT ---"));
        assert!(prompt.contains("--- END SELECTED TEXT ---"));
    }

    #[test]
    fn build_prompt_includes_conversation_history() {
        let history = vec![
            NoteAiChatMessage {
                role: "user".to_string(),
                content: "Fix the typo".to_string(),
            },
            NoteAiChatMessage {
                role: "assistant".to_string(),
                content: "Fixed it".to_string(),
            },
        ];

        let prompt = build_note_ai_prompt("note text", "selected", &history, "Now capitalize it");

        assert!(prompt.contains("\"role\":\"user\""));
        assert!(prompt.contains("\"content\":\"Fix the typo\""));
        assert!(prompt.contains("\"role\":\"assistant\""));
        assert!(prompt.contains("\"content\":\"Fixed it\""));
        assert!(prompt.contains("--- CONVERSATION HISTORY ---"));
        assert!(prompt.contains("--- END CONVERSATION HISTORY ---"));
    }

    #[test]
    fn build_prompt_mentions_image_syntax() {
        let prompt = build_note_ai_prompt("note", "selected", &[], "help");
        assert!(
            prompt.contains("!["),
            "prompt should mention image markdown syntax"
        );
    }
}
