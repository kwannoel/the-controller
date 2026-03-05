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
        "timeout 2 bash -c 'echo \"working:$THE_CONTROLLER_SESSION_ID\" | nc -U {}' 2>/dev/null; true",
        SOCKET_PATH
    );
    let idle_cmd = format!(
        "timeout 2 bash -c 'echo \"idle:$THE_CONTROLLER_SESSION_ID\" | nc -U {}' 2>/dev/null; true",
        SOCKET_PATH
    );

    serde_json::json!({
        "hooks": {
            "UserPromptSubmit": [{
                "type": "command",
                "command": working_cmd
            }],
            "Stop": [{
                "type": "command",
                "command": idle_cmd
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
        assert!(hooks.get("Stop").is_some());
        assert!(hooks.get("Notification").is_some());
    }

    #[test]
    fn test_parse_status_message_invalid_format() {
        assert!(parse_status_message("garbage").is_none());
        assert!(parse_status_message("working:not-a-uuid").is_none());
        assert!(parse_status_message("unknown:550e8400-e29b-41d4-a716-446655440000").is_none());
        assert!(parse_status_message("").is_none());
    }
}
