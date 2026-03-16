use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::emitter::EventEmitter;
use crate::state::AppState;
use crate::worktree::WorktreeManager;

const DEFAULT_SOCKET_PATH: &str = "/tmp/the-controller.sock";
const DEFAULT_STAGED_SOCKET_PATH: &str = "/tmp/the-controller-staged.sock";

/// Return the socket path used by staged Controller instances.
pub fn staged_socket_path() -> &'static str {
    DEFAULT_STAGED_SOCKET_PATH
}

/// Return the socket path, checking the CONTROLLER_SOCKET env var first.
pub fn socket_path() -> String {
    std::env::var("CONTROLLER_SOCKET").unwrap_or_else(|_| DEFAULT_SOCKET_PATH.to_string())
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

#[derive(Debug)]
enum SocketMessage {
    Status { status: String, session_id: Uuid },
    SecureEnv(crate::secure_env::SecureEnvRequest),
    Stage { project_id: Uuid, session_id: Uuid },
}

fn parse_socket_message(msg: &str) -> Result<SocketMessage, String> {
    if let Some((status, session_id)) = parse_status_message(msg) {
        return Ok(SocketMessage::Status {
            status: status.to_string(),
            session_id,
        });
    }

    if let Some(rest) = msg.strip_prefix("stage:") {
        let (proj_str, sess_str) = rest
            .split_once(':')
            .ok_or_else(|| "stage format: stage:<project_id>:<session_id>".to_string())?;
        let project_id = Uuid::parse_str(proj_str).map_err(|e| e.to_string())?;
        let session_id = Uuid::parse_str(sess_str).map_err(|e| e.to_string())?;
        return Ok(SocketMessage::Stage {
            project_id,
            session_id,
        });
    }

    let secure_env = msg
        .strip_prefix("secure-env:")
        .ok_or_else(|| "Unknown socket message".to_string())?;
    let request = crate::secure_env::parse_secure_env_request(secure_env)?;
    Ok(SocketMessage::SecureEnv(request))
}

fn write_socket_response(stream: &mut UnixStream, response: &crate::secure_env::SecureEnvResponse) {
    let line = format!(
        "{}\n",
        crate::secure_env::format_secure_env_response(response)
    );
    if let Err(err) = stream.write_all(line.as_bytes()) {
        eprintln!("Failed to write socket response: {}", err);
    }
}

fn dispatch_secure_env_request(
    state: &AppState,
    emitter: &Arc<dyn EventEmitter>,
    request: crate::secure_env::SecureEnvRequest,
    response_tx: std::sync::mpsc::SyncSender<crate::secure_env::SecureEnvResponse>,
) -> Result<(), crate::secure_env::SecureEnvResponse> {
    let pending = crate::secure_env::begin_secure_env_request_with_response(
        state,
        &request.project_selector,
        &request.key,
        &request.request_id,
        Some(response_tx),
    )
    .map_err(|err| crate::secure_env::SecureEnvResponse {
        kind: crate::secure_env::SecureEnvResponseKind::Error,
        status: match err.as_str() {
            "A secure env request is already active" => "busy".to_string(),
            _ if err.starts_with("Unknown project:") => "unknown-project".to_string(),
            _ => "invalid-request".to_string(),
        },
        request_id: request.request_id.clone(),
    })?;

    let payload = serde_json::json!({
        "requestId": pending.request_id,
        "projectId": pending.project_id,
        "projectName": pending.project_name,
        "key": pending.key,
    })
    .to_string();

    if emitter.emit("secure-env-requested", &payload).is_err() {
        if let Ok(mut active) = state.secure_env_request.lock() {
            if active
                .as_ref()
                .is_some_and(|request| request.pending.request_id == pending.request_id)
            {
                *active = None;
            }
        }

        return Err(crate::secure_env::SecureEnvResponse {
            kind: crate::secure_env::SecureEnvResponseKind::Error,
            status: "emit-failed".to_string(),
            request_id: request.request_id,
        });
    }

    Ok(())
}

/// Start the Unix domain socket listener.
/// Cleans up stale sockets, binds, and spawns a thread to accept connections.
pub fn start_listener(app_handle: AppHandle) {
    let path = socket_path();

    // Clean up stale socket
    if std::path::Path::new(&path).exists() {
        match UnixStream::connect(&path) {
            Ok(_) => {
                eprintln!(
                    "Warning: another instance appears to be running (socket {} is active)",
                    path
                );
                return;
            }
            Err(_) => {
                // Connection refused — stale socket, safe to remove
                let _ = std::fs::remove_file(&path);
            }
        }
    }

    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind Unix socket at {}: {}", path, e);
            return;
        }
    };

    let emitter: Arc<dyn EventEmitter> = match app_handle.try_state::<AppState>() {
        Some(s) => s.emitter.clone(),
        None => {
            eprintln!("status_socket: AppState not available");
            return;
        }
    };

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let app_handle = app_handle.clone();
                    let emitter = emitter.clone();
                    std::thread::spawn(move || {
                        handle_connection(stream, &app_handle, &emitter);
                    });
                }
                Err(e) => {
                    eprintln!("Error accepting connection on status socket: {}", e);
                }
            }
        }
    });
}

fn handle_connection(stream: UnixStream, app_handle: &AppHandle, emitter: &Arc<dyn EventEmitter>) {
    let mut writer = match stream.try_clone() {
        Ok(stream) => stream,
        Err(err) => {
            eprintln!("Failed to clone status socket stream: {}", err);
            return;
        }
    };
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        match line {
            Ok(msg) => {
                let msg = msg.trim();
                match parse_socket_message(msg) {
                    Ok(SocketMessage::Status { status, session_id }) => {
                        if status == "cleanup" {
                            handle_cleanup(app_handle, session_id);
                            return;
                        }
                        let event_name = format!("session-status-hook:{}", session_id);
                        if let Err(e) = emitter.emit(&event_name, &status) {
                            eprintln!("Failed to emit {}: {}", event_name, e);
                        }
                        if status == "idle" {
                            crate::auto_worker::notify_session_idle(session_id);
                        }
                    }
                    Ok(SocketMessage::Stage {
                        project_id,
                        session_id,
                    }) => {
                        let app_handle = app_handle.clone();
                        let (tx, rx) = std::sync::mpsc::sync_channel(1);
                        tauri::async_runtime::spawn(async move {
                            let state = app_handle.state::<AppState>();
                            let result = crate::commands::stage_session_core(
                                &state, project_id, session_id, false,
                            )
                            .await;
                            let _ = tx.send(result);
                        });
                        let response = match rx.recv() {
                            Ok(Ok(port)) => format!("staged:{}\n", port),
                            Ok(Err(e)) => format!("error:{}\n", e),
                            Err(_) => "error:internal channel error\n".to_string(),
                        };
                        if let Err(e) = writer.write_all(response.as_bytes()) {
                            eprintln!("Failed to write stage response: {}", e);
                        }
                        return;
                    }
                    Ok(SocketMessage::SecureEnv(request)) => {
                        let Some(state) = app_handle.try_state::<AppState>() else {
                            write_socket_response(
                                &mut writer,
                                &crate::secure_env::SecureEnvResponse {
                                    kind: crate::secure_env::SecureEnvResponseKind::Error,
                                    status: "app-state-unavailable".to_string(),
                                    request_id: request.request_id,
                                },
                            );
                            return;
                        };

                        let (response_tx, response_rx) = std::sync::mpsc::sync_channel(1);
                        match dispatch_secure_env_request(&state, emitter, request, response_tx) {
                            Ok(()) => match response_rx.recv() {
                                Ok(response) => write_socket_response(&mut writer, &response),
                                Err(err) => {
                                    eprintln!("Failed to receive secure env response: {}", err);
                                    write_socket_response(
                                        &mut writer,
                                        &crate::secure_env::SecureEnvResponse {
                                            kind: crate::secure_env::SecureEnvResponseKind::Error,
                                            status: "response-channel-closed".to_string(),
                                            request_id: "unknown".to_string(),
                                        },
                                    );
                                }
                            },
                            Err(response) => write_socket_response(&mut writer, &response),
                        }
                        return;
                    }
                    Err(err) if msg.starts_with("secure-env:") => {
                        eprintln!("Invalid secure env socket message: {}", err);
                        write_socket_response(
                            &mut writer,
                            &crate::secure_env::SecureEnvResponse {
                                kind: crate::secure_env::SecureEnvResponseKind::Error,
                                status: "invalid-request".to_string(),
                                request_id: "unknown".to_string(),
                            },
                        );
                        return;
                    }
                    Err(_) => {}
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
        if let Ok(inventory) = storage.list_projects() {
            inventory.warn_if_corrupt("status socket cleanup");
            let mut projects = inventory.projects;
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
                        if let Err(e) =
                            WorktreeManager::remove_worktree(wt_path, &project.repo_path, branch)
                        {
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
    if let Err(e) = state.emitter.emit(&event_name, "cleanup") {
        eprintln!("Failed to emit {}: {}", event_name, e);
    }
}

/// Generate a JSON settings string for Claude Code's `--settings` flag.
/// Configures hooks that report session status changes over the Unix socket.
pub fn hook_settings_json(session_id: Uuid) -> String {
    let path = socket_path();
    let working_cmd = format!(
        "echo \"working:{}\" | nc -U -w 2 {} 2>/dev/null; true",
        session_id, path
    );
    let idle_cmd = format!(
        "echo \"idle:{}\" | nc -U -w 2 {} 2>/dev/null; true",
        session_id, path
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
    let _ = std::fs::remove_file(socket_path());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Mutex;

    use tempfile::TempDir;
    use uuid::Uuid;

    use crate::emitter::EventEmitter;
    use crate::models::{AutoWorkerConfig, MaintainerConfig, Project};
    use crate::secure_env::{
        ActiveSecureEnvRequest, PendingSecureEnvRequest, SecureEnvResponse, SecureEnvResponseKind,
    };
    use crate::state::AppState;
    use crate::storage::Storage;

    struct RecordingEmitter {
        events: Mutex<Vec<(String, String)>>,
    }

    impl RecordingEmitter {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                events: Mutex::new(Vec::new()),
            })
        }
    }

    impl EventEmitter for RecordingEmitter {
        fn emit(&self, event: &str, payload: &str) -> Result<(), String> {
            self.events
                .lock()
                .unwrap()
                .push((event.to_string(), payload.to_string()));
            Ok(())
        }
    }

    struct FailingEmitter;

    impl FailingEmitter {
        fn new() -> Arc<Self> {
            Arc::new(Self)
        }
    }

    impl EventEmitter for FailingEmitter {
        fn emit(&self, _event: &str, _payload: &str) -> Result<(), String> {
            Err("emit failed".to_string())
        }
    }

    fn make_app_state(tmp: &TempDir) -> AppState {
        AppState::from_storage(
            Storage::new(tmp.path().to_path_buf()),
            crate::emitter::NoopEmitter::new(),
        )
        .unwrap()
    }

    fn save_project(state: &AppState, name: &str, repo_path: PathBuf) {
        let project = Project {
            id: Uuid::new_v4(),
            name: name.to_string(),
            repo_path: repo_path.to_string_lossy().to_string(),
            created_at: "2026-03-10T00:00:00Z".to_string(),
            archived: false,
            maintainer: MaintainerConfig::default(),
            auto_worker: AutoWorkerConfig::default(),
            prompts: vec![],
            sessions: vec![],
            staged_session: None,
        };

        state
            .storage
            .lock()
            .unwrap()
            .save_project(&project)
            .unwrap();
    }

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
        for event_name in &[
            "UserPromptSubmit",
            "PreToolUse",
            "PostToolUse",
            "Stop",
            "Notification",
        ] {
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
    fn test_socket_path_returns_default() {
        // When CONTROLLER_SOCKET is not set, returns the default path
        if std::env::var("CONTROLLER_SOCKET").is_err() {
            assert_eq!(socket_path(), "/tmp/the-controller.sock");
        }
    }

    #[test]
    fn test_socket_path_respects_env_var() {
        // Temporarily set the env var and verify socket_path reads it.
        // Note: env vars are process-global, but test runners run tests
        // in separate threads. This is acceptable for a quick read-check.
        let key = "CONTROLLER_SOCKET";
        let original = std::env::var(key).ok();
        std::env::set_var(key, "/tmp/custom-controller.sock");
        assert_eq!(socket_path(), "/tmp/custom-controller.sock");
        match original {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
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

    #[test]
    fn parses_secure_env_message() {
        let message =
            parse_socket_message("secure-env:set|demo-project|OPENAI_API_KEY|req-123").unwrap();

        match message {
            SocketMessage::SecureEnv(request) => {
                assert_eq!(request.project_selector, "demo-project");
                assert_eq!(request.key, "OPENAI_API_KEY");
                assert_eq!(request.request_id, "req-123");
            }
            other => panic!("expected secure env request, got {other:?}"),
        }
    }

    #[test]
    fn dispatches_secure_env_request_and_emits_frontend_event() {
        let tmp = TempDir::new().unwrap();
        let state = make_app_state(&tmp);
        let repo_path = tmp.path().join("demo-project");
        std::fs::create_dir_all(&repo_path).unwrap();
        save_project(&state, "demo-project", repo_path);
        let emitter = RecordingEmitter::new();
        let (tx, _rx) = std::sync::mpsc::sync_channel(1);
        let request =
            crate::secure_env::parse_secure_env_request("set|demo-project|OPENAI_API_KEY|req-123")
                .unwrap();

        dispatch_secure_env_request(
            &state,
            &(emitter.clone() as Arc<dyn EventEmitter>),
            request,
            tx,
        )
        .unwrap();

        let events = emitter.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, "secure-env-requested");
        assert!(events[0].1.contains("\"projectName\":\"demo-project\""));
        assert!(events[0].1.contains("\"key\":\"OPENAI_API_KEY\""));
        assert!(state.secure_env_request.lock().unwrap().is_some());
    }

    #[test]
    fn returns_busy_error_response_for_second_secure_env_request() {
        let tmp = TempDir::new().unwrap();
        let state = make_app_state(&tmp);
        let repo_path = tmp.path().join("demo-project");
        std::fs::create_dir_all(&repo_path).unwrap();
        save_project(&state, "demo-project", repo_path);
        let emitter = RecordingEmitter::new();
        let (tx, _rx) = std::sync::mpsc::sync_channel(1);

        *state.secure_env_request.lock().unwrap() = Some(ActiveSecureEnvRequest {
            pending: PendingSecureEnvRequest {
                request_id: "req-000".to_string(),
                project_id: Uuid::new_v4(),
                project_name: "demo-project".to_string(),
                env_path: tmp.path().join("demo-project/.env"),
                key: "OPENAI_API_KEY".to_string(),
            },
            response_tx: None,
        });

        let request = crate::secure_env::parse_secure_env_request(
            "set|demo-project|ANTHROPIC_API_KEY|req-123",
        )
        .unwrap();

        let response =
            dispatch_secure_env_request(&state, &(emitter as Arc<dyn EventEmitter>), request, tx)
                .unwrap_err();

        assert_eq!(
            response,
            SecureEnvResponse {
                kind: SecureEnvResponseKind::Error,
                status: "busy".to_string(),
                request_id: "req-123".to_string(),
            }
        );
    }

    #[test]
    fn clears_active_request_when_secure_env_emit_fails() {
        let tmp = TempDir::new().unwrap();
        let state = make_app_state(&tmp);
        let repo_path = tmp.path().join("demo-project");
        std::fs::create_dir_all(&repo_path).unwrap();
        save_project(&state, "demo-project", repo_path);
        let emitter = FailingEmitter::new();
        let (tx, _rx) = std::sync::mpsc::sync_channel(1);
        let request =
            crate::secure_env::parse_secure_env_request("set|demo-project|OPENAI_API_KEY|req-123")
                .unwrap();

        let response =
            dispatch_secure_env_request(&state, &(emitter as Arc<dyn EventEmitter>), request, tx)
                .unwrap_err();

        assert_eq!(
            response,
            SecureEnvResponse {
                kind: SecureEnvResponseKind::Error,
                status: "emit-failed".to_string(),
                request_id: "req-123".to_string(),
            }
        );
        assert!(state.secure_env_request.lock().unwrap().is_none());
    }
}
