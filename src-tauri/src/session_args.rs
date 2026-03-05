use uuid::Uuid;

const CODEX_FULL_PERMISSION_ARGS: [&str; 4] = [
    "--sandbox",
    "danger-full-access",
    "--ask-for-approval",
    "never",
];

/// Build command-line arguments for spawned assistant sessions.
/// Keeps Claude-specific hooks and applies full permissions for Codex.
/// When an `initial_prompt` is provided (e.g. from a GitHub issue), it is
/// injected as system context AND as a positional user prompt so the
/// assistant starts working immediately without waiting for user input.
pub fn build_session_args(
    command: &str,
    session_id: Uuid,
    continue_session: bool,
    initial_prompt: Option<&str>,
) -> Vec<String> {
    let mut args = Vec::new();

    if continue_session {
        args.push("--continue".to_string());
    }

    match command {
        "claude" => {
            let settings_json = crate::status_socket::hook_settings_json(session_id);
            args.push("--settings".to_string());
            args.push(settings_json);
            if let Some(prompt) = initial_prompt {
                args.push("--append-system-prompt".to_string());
                args.push(prompt.to_string());
            }
        }
        "codex" => {
            args.extend(CODEX_FULL_PERMISSION_ARGS.iter().map(|s| s.to_string()));
        }
        _ => {}
    }

    // Positional prompt (must come after all flags) so the assistant
    // begins working immediately when an issue is attached.
    if let Some(prompt) = initial_prompt {
        args.push(prompt.to_string());
    }

    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_args_include_full_permissions() {
        let session_id = Uuid::new_v4();
        let args = build_session_args("codex", session_id, false, None);
        assert_eq!(
            args,
            vec![
                "--sandbox".to_string(),
                "danger-full-access".to_string(),
                "--ask-for-approval".to_string(),
                "never".to_string()
            ]
        );
    }

    #[test]
    fn codex_args_preserve_continue_flag() {
        let session_id = Uuid::new_v4();
        let args = build_session_args("codex", session_id, true, None);
        assert_eq!(
            args,
            vec![
                "--continue".to_string(),
                "--sandbox".to_string(),
                "danger-full-access".to_string(),
                "--ask-for-approval".to_string(),
                "never".to_string()
            ]
        );
    }

    #[test]
    fn claude_args_include_settings_and_prompt() {
        let session_id = Uuid::new_v4();
        let args = build_session_args("claude", session_id, false, Some("fix this"));
        assert_eq!(args[0], "--settings");
        let parsed: serde_json::Value = serde_json::from_str(&args[1]).unwrap();
        assert!(parsed.get("hooks").is_some());
        assert_eq!(args[2], "--append-system-prompt");
        assert_eq!(args[3], "fix this");
        // Positional prompt at end so claude auto-starts
        assert_eq!(args[4], "fix this");
    }

    #[test]
    fn codex_args_include_positional_prompt_when_issue_attached() {
        let session_id = Uuid::new_v4();
        let args = build_session_args("codex", session_id, false, Some("fix this"));
        assert_eq!(
            args,
            vec![
                "--sandbox".to_string(),
                "danger-full-access".to_string(),
                "--ask-for-approval".to_string(),
                "never".to_string(),
                "fix this".to_string(),
            ]
        );
    }

    #[test]
    fn no_positional_prompt_when_no_issue() {
        let session_id = Uuid::new_v4();
        let claude_args = build_session_args("claude", session_id, false, None);
        // Only --settings and its value, no positional prompt
        assert_eq!(claude_args.len(), 2);

        let codex_args = build_session_args("codex", session_id, false, None);
        // Only the 4 permission args, no positional prompt
        assert_eq!(codex_args.len(), 4);
    }
}
