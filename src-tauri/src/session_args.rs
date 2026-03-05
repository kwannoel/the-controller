use uuid::Uuid;

const CODEX_FULL_PERMISSION_ARGS: [&str; 4] = [
    "--sandbox",
    "danger-full-access",
    "--ask-for-approval",
    "never",
];

/// Build command-line arguments for spawned assistant sessions.
/// Keeps Claude-specific hooks and applies full permissions for Codex.
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
                args.push("--prompt".to_string());
                args.push(prompt.to_string());
            }
        }
        "codex" => {
            args.extend(CODEX_FULL_PERMISSION_ARGS.iter().map(|s| s.to_string()));
        }
        _ => {}
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
        assert_eq!(args[2], "--prompt");
        assert_eq!(args[3], "fix this");
    }
}
