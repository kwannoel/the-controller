use uuid::Uuid;

const BACKGROUND_WORKFLOW_SUFFIX: &str = "\n\nYou are an autonomous background worker. Complete the following workflow end-to-end without waiting for user input:\n1. **Design** — Analyze the issue and plan the approach\n2. **Implement** — Write the code changes\n3. **Review** — Self-review the changes for correctness and quality\n4. **Push PR** — Create and push a pull request\n5. **Merge** — Merge the PR once checks pass\n6. **Report** — If step 5 succeeded, post a report comment on the GitHub issue (use the issue number from above) via `gh issue comment <issue_number> --body \"...\"`. Start the comment body with the marker `<!-- auto-worker-report -->` on its own line, then summarize what was changed, include the PR URL, and confirm the merge succeeded. Skip this step if the merge did not succeed.\n7. **Finalize issue state** — Before syncing local master, update the GitHub issue to reflect the final worker outcome. Remove `in-progress`. If the issue is now closed, keep or add `assigned-to-auto-worker`. If the issue is still open, remove `assigned-to-auto-worker` so the issue can be retried cleanly.\n8. **Sync local master** — After the GitHub issue state is finalized, sync master in the main repo (where master is always checked out) by running: `git -C \"$(git worktree list | head -1 | awk '{print $1}')\" pull`\n\nCOMMIT TAGGING: Every commit you create MUST include the trailer `Contributed-by: auto-worker` at the end of the commit message body. This is how we identify worker contributions. Example:\n```\nfix: resolve parsing edge case\n\nHandles the null input scenario.\n\nContributed-by: auto-worker\n```\n\nCRITICAL: Never ask questions. Never wait for confirmation or user input. If you are uncertain about anything, make your best judgment and proceed. You must complete the entire workflow autonomously.";

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

    match command {
        "claude" => {
            if continue_session {
                args.push("--continue".to_string());
            }
            let settings_json = crate::status_socket::hook_settings_json(session_id);
            args.push("--settings".to_string());
            args.push(settings_json);
            if let Some(prompt) = initial_prompt {
                args.push("--append-system-prompt".to_string());
                args.push(prompt.to_string());
            }
        }
        "codex" => {
            if continue_session {
                args.push("resume".to_string());
                args.push("--last".to_string());
            }
            args.extend(CODEX_FULL_PERMISSION_ARGS.iter().map(|s| s.to_string()));
        }
        _ => {
            if continue_session {
                args.push("--continue".to_string());
            }
        }
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
    fn codex_args_use_resume_subcommand() {
        let session_id = Uuid::new_v4();
        let args = build_session_args("codex", session_id, true, None);
        assert_eq!(
            args,
            vec![
                "resume".to_string(),
                "--last".to_string(),
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
        let prompt = build_issue_prompt(
            42,
            "Fix the bug",
            "https://github.com/foo/bar/issues/42",
            false,
        );
        assert!(prompt.contains("GitHub issue #42: Fix the bug"));
        assert!(prompt.contains("closes #42"));
        assert!(!prompt.contains("autonomous background worker"));
    }

    #[test]
    fn build_issue_prompt_with_background() {
        let prompt = build_issue_prompt(
            42,
            "Fix the bug",
            "https://github.com/foo/bar/issues/42",
            true,
        );
        assert!(prompt.contains("GitHub issue #42: Fix the bug"));
        assert!(prompt.contains("closes #42"));
        assert!(prompt.contains("autonomous background worker"));
        assert!(prompt.contains("Design"));
        assert!(prompt.contains("Implement"));
        assert!(prompt.contains("Push PR"));
        assert!(prompt.contains("Merge"));
        assert!(prompt.contains("Report"));
        assert!(prompt.contains("gh issue comment"));
        assert!(prompt.contains("auto-worker-report"));
        assert!(prompt.contains("Finalize issue state"));
        assert!(prompt.contains("Sync local master"));
        assert!(prompt.contains("Never ask questions"));
        assert!(prompt.contains("Contributed-by: auto-worker"));
    }

    #[test]
    fn build_issue_prompt_finalizes_issue_before_syncing_local_master() {
        let prompt = build_issue_prompt(
            42,
            "Fix the bug",
            "https://github.com/foo/bar/issues/42",
            true,
        );
        let finalize_index = prompt.find("Finalize issue state").unwrap();
        let sync_index = prompt.find("Sync local master").unwrap();

        assert!(finalize_index < sync_index);
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

    #[test]
    fn unknown_command_produces_empty_args() {
        let session_id = Uuid::new_v4();
        let args = build_session_args("unknown-tool", session_id, false, None);
        assert!(args.is_empty());
    }

    #[test]
    fn unknown_command_with_continue_only_has_continue() {
        let session_id = Uuid::new_v4();
        let args = build_session_args("unknown-tool", session_id, true, None);
        assert_eq!(args, vec!["--continue".to_string()]);
    }

    #[test]
    fn claude_args_continue_plus_prompt() {
        let session_id = Uuid::new_v4();
        let args = build_session_args("claude", session_id, true, Some("do stuff"));
        assert_eq!(args[0], "--continue");
        assert_eq!(args[1], "--settings");
        // args[2] is settings JSON
        assert_eq!(args[3], "--append-system-prompt");
        assert_eq!(args[4], "do stuff");
        assert_eq!(args[5], "do stuff"); // positional prompt at end
    }
}
