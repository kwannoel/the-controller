use std::io::{BufRead, BufReader};
use std::os::unix::net::{UnixListener, UnixStream};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::state::AppState;
use crate::worktree::WorktreeManager;

const SOCKET_PATH: &str = "/tmp/the-controller.sock";

/// Return the socket path constant.
pub fn socket_path() -> &'static str {
    SOCKET_PATH
}

/// Parse a "working:uuid", "idle:uuid", or "cleanup:uuid" message.
/// Returns None for anything that doesn't match.
pub fn parse_status_message(msg: &str) -> Option<(&str, Uuid)> {
    let (status, id_str) = msg.split_once(':')?;
    match status {
        "working" | "idle" | "cleanup" => {
            let id = Uuid::parse_str(id_str).ok()?;
            Some((status, id))
        }
        _ => None,
    }
}

/// Start the Unix domain socket listener.
/// Cleans up stale sockets, binds, and spawns a thread to accept connections.
pub fn start_listener(app_handle: AppHandle) {
    // Clean up stale socket
    if std::path::Path::new(SOCKET_PATH).exists() {
        match UnixStream::connect(SOCKET_PATH) {
            Ok(_) => {
                eprintln!(
                    "Warning: another instance appears to be running (socket {} is active)",
                    SOCKET_PATH
                );
                return;
            }
            Err(_) => {
                // Connection refused — stale socket, safe to remove
                let _ = std::fs::remove_file(SOCKET_PATH);
            }
        }
    }

    let listener = match UnixListener::bind(SOCKET_PATH) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind Unix socket at {}: {}", SOCKET_PATH, e);
            return;
        }
    };

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let app_handle = app_handle.clone();
                    std::thread::spawn(move || {
                        handle_connection(stream, &app_handle);
                    });
                }
                Err(e) => {
                    eprintln!("Error accepting connection on status socket: {}", e);
                }
            }
        }
    });
}

fn handle_connection(stream: UnixStream, app_handle: &AppHandle) {
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        match line {
            Ok(msg) => {
                let msg = msg.trim();
                if let Some((status, session_id)) = parse_status_message(msg) {
                    if status == "cleanup" {
                        handle_cleanup(app_handle, session_id);
                        return; // close connection immediately
                    } else {
                        let event_name = format!("session-status-hook:{}", session_id);
                        if let Err(e) = app_handle.emit(&event_name, status) {
                            eprintln!("Failed to emit {}: {}", event_name, e);
                        }
                        if status == "idle" {
                            crate::auto_worker::notify_session_idle(session_id);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading from status socket connection: {}", e);
                break;
            }
        }
    }
}

/// Handle a cleanup message by deleting the session and worktree directly
/// on the backend, then telling the frontend to refresh.
fn handle_cleanup(app_handle: &AppHandle, session_id: Uuid) {
    let state = match app_handle.try_state::<AppState>() {
        Some(s) => s,
        None => {
            eprintln!("cleanup: AppState not available");
            return;
        }
    };

    // Find and remove the session from its project, delete worktree
    // IMPORTANT: acquire storage lock BEFORE pty_manager to match
    // the lock ordering used by Tauri commands (storage → pty_manager).
    // Reversed order causes deadlock.
    if let Ok(storage) = state.storage.lock() {
        if let Ok(mut projects) = storage.list_projects() {
            for project in &mut projects {
                if let Some(pos) = project.sessions.iter().position(|s| s.id == session_id) {
                    let session = project.sessions.remove(pos);
                    if let Err(e) = storage.save_project(project) {
                        eprintln!("cleanup: failed to save project: {}", e);
                    }
                    // Delete the worktree
                    if let (Some(wt_path), Some(branch)) =
                        (&session.worktree_path, &session.worktree_branch)
                    {
                        if let Err(e) = WorktreeManager::remove_worktree(
                            wt_path,
                            &project.repo_path,
                            branch,
                        ) {
                            eprintln!("cleanup: failed to remove worktree: {}", e);
                        }
                    }
                    break;
                }
            }
        }
    }

    // Close the PTY / kill tmux session (after releasing storage lock)
    if let Ok(mut pty_manager) = state.pty_manager.lock() {
        let _ = pty_manager.close_session(session_id);
    }

    // Tell the frontend to refresh its project list
    let event_name = format!("session-cleanup:{}", session_id);
    if let Err(e) = app_handle.emit(&event_name, "cleanup") {
        eprintln!("Failed to emit {}: {}", event_name, e);
    }
}

/// Generate a JSON settings string for Claude Code's `--settings` flag.
/// Configures hooks that report session status changes over the Unix socket.
pub fn hook_settings_json(session_id: Uuid) -> String {
    let working_cmd = format!(
        "echo \"working:{}\" | nc -U -w 2 {} 2>/dev/null; true",
        session_id, SOCKET_PATH
    );
    let idle_cmd = format!(
        "echo \"idle:{}\" | nc -U -w 2 {} 2>/dev/null; true",
        session_id, SOCKET_PATH
    );

    serde_json::json!({
        "hooks": {
            "UserPromptSubmit": [{
                "hooks": [{
                    "type": "command",
                    "command": working_cmd
                }]
            }],
            "PreToolUse": [{
                "hooks": [{
                    "type": "command",
                    "command": working_cmd
                }]
            }],
            "PostToolUse": [{
                "hooks": [{
                    "type": "command",
                    "command": working_cmd
                }]
            }],
            "Stop": [{
                "hooks": [{
                    "type": "command",
                    "command": idle_cmd
                }]
            }],
            "Notification": [{
                "matcher": "idle_prompt",
                "hooks": [{
                    "type": "command",
                    "command": idle_cmd
                }]
            }]
        }
    })
    .to_string()
}

/// Remove the socket file. Call on app shutdown.
pub fn cleanup() {
    let _ = std::fs::remove_file(SOCKET_PATH);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_status_message_working() {
        let id = uuid::Uuid::new_v4();
        let msg = format!("working:{}", id);
        let (status, parsed_id) = parse_status_message(&msg).unwrap();
        assert_eq!(status, "working");
        assert_eq!(parsed_id, id);
    }

    #[test]
    fn test_parse_status_message_idle() {
        let id = uuid::Uuid::new_v4();
        let msg = format!("idle:{}", id);
        let (status, parsed_id) = parse_status_message(&msg).unwrap();
        assert_eq!(status, "idle");
        assert_eq!(parsed_id, id);
    }

    #[test]
    fn test_hook_settings_json_is_valid() {
        let id = uuid::Uuid::new_v4();
        let json = hook_settings_json(id);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let hooks = parsed.get("hooks").unwrap();
        assert!(hooks.get("UserPromptSubmit").is_some());
        assert!(hooks.get("PreToolUse").is_some());
        assert!(hooks.get("PostToolUse").is_some());
        assert!(hooks.get("Stop").is_some());
        assert!(hooks.get("Notification").is_some());

        // Verify new hooks format: each event entry must have a nested "hooks" array
        for event_name in &["UserPromptSubmit", "PreToolUse", "PostToolUse", "Stop", "Notification"] {
            let entries = hooks.get(*event_name).unwrap().as_array().unwrap();
            for entry in entries {
                assert!(
                    entry.get("hooks").is_some(),
                    "{} entry missing nested 'hooks' array",
                    event_name
                );
                let inner = entry.get("hooks").unwrap().as_array().unwrap();
                assert!(!inner.is_empty(), "{} has empty hooks array", event_name);
            }
        }
    }

    #[test]
    fn test_hook_commands_use_nc_timeout_not_timeout_binary() {
        // macOS doesn't have `timeout` — hook commands must use `nc -w` instead
        let id = uuid::Uuid::new_v4();
        let json = hook_settings_json(id);
        assert!(
            !json.contains("timeout "),
            "hook commands must not use `timeout` (not available on macOS)"
        );
        assert!(
            json.contains("nc -U -w 2"),
            "hook commands must use `nc -w 2` for timeout"
        );
    }

    #[test]
    fn test_parse_status_message_cleanup() {
        let id = uuid::Uuid::new_v4();
        let msg = format!("cleanup:{}", id);
        let (status, parsed_id) = parse_status_message(&msg).unwrap();
        assert_eq!(status, "cleanup");
        assert_eq!(parsed_id, id);
    }

    #[test]
    fn test_parse_status_message_invalid_format() {
        assert!(parse_status_message("garbage").is_none());
        assert!(parse_status_message("working:not-a-uuid").is_none());
        assert!(parse_status_message("unknown:550e8400-e29b-41d4-a716-446655440000").is_none());
        assert!(parse_status_message("").is_none());
    }

    #[test]
    fn test_socket_path_returns_expected() {
        assert_eq!(socket_path(), "/tmp/the-controller.sock");
    }

    #[test]
    fn test_hook_commands_contain_hardcoded_session_id() {
        let id = uuid::Uuid::new_v4();
        let json = hook_settings_json(id);
        let id_str = id.to_string();
        assert!(
            json.contains(&format!("working:{}", id_str)),
            "hook commands must contain hardcoded session UUID, not env var"
        );
        assert!(
            json.contains(&format!("idle:{}", id_str)),
            "hook commands must contain hardcoded session UUID, not env var"
        );
        assert!(
            !json.contains("$THE_CONTROLLER_SESSION_ID"),
            "hook commands must not rely on env var (not available in tmux sessions)"
        );
    }

    #[test]
    fn test_hook_settings_json_contains_socket_path() {
        let id = uuid::Uuid::new_v4();
        let json = hook_settings_json(id);
        assert!(json.contains("/tmp/the-controller.sock"));
    }

    #[test]
    fn test_parse_status_message_with_extra_colons() {
        // UUID contains hyphens not colons, but test split_once behavior
        // "working:550e8400-e29b-41d4-a716-446655440000" should work
        let msg = "working:550e8400-e29b-41d4-a716-446655440000";
        let result = parse_status_message(msg);
        assert!(result.is_some());
        let (status, _) = result.unwrap();
        assert_eq!(status, "working");
    }
}
