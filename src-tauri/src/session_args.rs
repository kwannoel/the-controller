use uuid::Uuid;

const BACKGROUND_WORKFLOW_SUFFIX: &str = "\n\nYou are an autonomous background worker. Complete the following workflow end-to-end without waiting for user input:\n1. **Design** — Analyze the issue and plan the approach\n2. **Implement** — Write the code changes\n3. **Review** — Self-review the changes for correctness and quality\n4. **Push PR** — Create and push a pull request\n5. **Merge** — Merge the PR once checks pass\n6. **Sync local master** — Pull merged changes to local master";

/// Build the initial prompt injected into a session from a GitHub issue.
/// When `background` is true, appends the autonomous workflow instructions.
pub fn build_issue_prompt(issue_number: u64, title: &str, url: &str, background: bool) -> String {
    let base = format!(
        "You are working on GitHub issue #{}: {}\nIssue URL: {}\nPlease include 'closes #{}' in any PR descriptions or final commit messages.",
        issue_number, title, url, issue_number
    );
    if background {
        format!("{}{}", base, BACKGROUND_WORKFLOW_SUFFIX)
    } else {
        base
    }
}

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
    fn build_issue_prompt_without_background() {
        let prompt = build_issue_prompt(42, "Fix the bug", "https://github.com/foo/bar/issues/42", false);
        assert!(prompt.contains("GitHub issue #42: Fix the bug"));
        assert!(prompt.contains("closes #42"));
        assert!(!prompt.contains("autonomous background worker"));
    }

    #[test]
    fn build_issue_prompt_with_background() {
        let prompt = build_issue_prompt(42, "Fix the bug", "https://github.com/foo/bar/issues/42", true);
        assert!(prompt.contains("GitHub issue #42: Fix the bug"));
        assert!(prompt.contains("closes #42"));
        assert!(prompt.contains("autonomous background worker"));
        assert!(prompt.contains("Design"));
        assert!(prompt.contains("Implement"));
        assert!(prompt.contains("Push PR"));
        assert!(prompt.contains("Merge"));
        assert!(prompt.contains("Sync local master"));
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
