use std::io::{BufRead, BufReader};
use std::os::unix::net::{UnixListener, UnixStream};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

const SOCKET_PATH: &str = "/tmp/the-controller.sock";

/// Return the socket path constant.
pub fn socket_path() -> &'static str {
    SOCKET_PATH
}

/// Parse a "working:uuid" or "idle:uuid" message.
/// Returns None for anything that doesn't match.
pub fn parse_status_message(msg: &str) -> Option<(&str, Uuid)> {
    let (status, id_str) = msg.split_once(':')?;
    match status {
        "working" | "idle" => {
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
                    let event_name = format!("session-status-hook:{}", session_id);
                    if let Err(e) = app_handle.emit(&event_name, status) {
                        eprintln!("Failed to emit {}: {}", event_name, e);
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

/// Generate a JSON settings string for Claude Code's `--settings` flag.
/// Configures hooks that report session status changes over the Unix socket.
pub fn hook_settings_json(_session_id: Uuid) -> String {
    let working_cmd = format!(
        "echo \"working:$THE_CONTROLLER_SESSION_ID\" | nc -U -w 2 {} 2>/dev/null; true",
        SOCKET_PATH
    );
    let idle_cmd = format!(
        "echo \"idle:$THE_CONTROLLER_SESSION_ID\" | nc -U -w 2 {} 2>/dev/null; true",
        SOCKET_PATH
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
