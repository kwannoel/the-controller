use base64::Engine;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::tmux::TmuxManager;

pub struct PtySession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    alive: Arc<Mutex<bool>>,
    tmux_session: bool,
}

pub struct PtyManager {
    sessions: HashMap<Uuid, PtySession>,
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Spawn a session. Uses tmux if available (survives dev restarts),
    /// otherwise falls back to a direct PTY (production path).
    /// When `continue_session` is true, passes `--continue` to claude to resume
    /// the last conversation in the working directory.
    pub fn spawn_session(
        &mut self,
        session_id: Uuid,
        working_dir: &str,
        kind: &str,
        app_handle: AppHandle,
        continue_session: bool,
        initial_prompt: Option<&str>,
    ) -> Result<(), String> {
        let command = match kind {
            "codex" => "codex",
            _ => "claude",
        };
        if TmuxManager::is_available() {
            // Create tmux session if it doesn't already exist
            if !TmuxManager::has_session(session_id) {
                TmuxManager::create_session(session_id, working_dir, command, continue_session, initial_prompt)?;
            }
            // Attach to the tmux session via a local PTY
            self.attach_tmux_session(session_id, app_handle)
        } else {
            // No tmux — spawn the command directly in a PTY
            self.spawn_direct_session(session_id, working_dir, command, app_handle, initial_prompt)
        }
    }

    /// Spawn a command directly in a local PTY without tmux.
    fn spawn_direct_session(
        &mut self,
        session_id: Uuid,
        working_dir: &str,
        command: &str,
        app_handle: AppHandle,
        initial_prompt: Option<&str>,
    ) -> Result<(), String> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("failed to open pty: {}", e))?;

        let mut cmd = CommandBuilder::new(command);
        cmd.cwd(working_dir);
        cmd.env_remove("CLAUDECODE");
        cmd.env("THE_CONTROLLER_SESSION_ID", session_id.to_string());
        if command == "claude" {
            let settings_json = crate::status_socket::hook_settings_json(session_id);
            cmd.arg("--settings");
            cmd.arg(settings_json);
            if let Some(prompt) = initial_prompt {
                cmd.arg("--prompt");
                cmd.arg(prompt);
            }
        }

        let _child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("failed to spawn {}: {}", command, e))?;

        drop(pair.slave);

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("failed to get pty writer: {}", e))?;

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("failed to get pty reader: {}", e))?;

        let alive = Arc::new(Mutex::new(true));
        let alive_clone = Arc::clone(&alive);

        let output_event = format!("pty-output:{}", session_id);
        let status_event = format!("session-status-changed:{}", session_id);

        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        if let Ok(mut a) = alive_clone.lock() {
                            *a = false;
                        }
                        let _ = app_handle.emit(&status_event, "idle");
                        break;
                    }
                    Ok(n) => {
                        let encoded =
                            base64::engine::general_purpose::STANDARD.encode(&buf[..n]);
                        let _ = app_handle.emit(&output_event, encoded);
                    }
                    Err(_) => {
                        if let Ok(mut a) = alive_clone.lock() {
                            *a = false;
                        }
                        let _ = app_handle.emit(&status_event, "idle");
                        break;
                    }
                }
            }
        });

        let session = PtySession {
            master: pair.master,
            writer,
            alive,
            tmux_session: false,
        };

        self.sessions.insert(session_id, session);
        Ok(())
    }

    /// Attach to an existing tmux session by spawning `tmux attach` in a local PTY.
    fn attach_tmux_session(
        &mut self,
        session_id: Uuid,
        app_handle: AppHandle,
    ) -> Result<(), String> {
        let tmux_name = TmuxManager::session_name(session_id);

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("failed to open pty: {}", e))?;

        let mut cmd = CommandBuilder::new("/opt/homebrew/bin/tmux");
        cmd.arg("attach-session");
        cmd.arg("-t");
        cmd.arg(&tmux_name);

        let _child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("failed to spawn tmux attach: {}", e))?;

        drop(pair.slave);

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("failed to get pty writer: {}", e))?;

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("failed to get pty reader: {}", e))?;

        let alive = Arc::new(Mutex::new(true));
        let alive_clone = Arc::clone(&alive);

        let output_event = format!("pty-output:{}", session_id);
        let status_event = format!("session-status-changed:{}", session_id);

        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        if let Ok(mut a) = alive_clone.lock() {
                            *a = false;
                        }
                        let _ = app_handle.emit(&status_event, "idle");
                        break;
                    }
                    Ok(n) => {
                        let encoded =
                            base64::engine::general_purpose::STANDARD.encode(&buf[..n]);
                        let _ = app_handle.emit(&output_event, encoded);
                    }
                    Err(_) => {
                        if let Ok(mut a) = alive_clone.lock() {
                            *a = false;
                        }
                        let _ = app_handle.emit(&status_event, "idle");
                        break;
                    }
                }
            }
        });

        let session = PtySession {
            master: pair.master,
            writer,
            alive,
            tmux_session: true,
        };

        self.sessions.insert(session_id, session);
        Ok(())
    }

    /// Spawn a direct (non-tmux) command. Used for short-lived commands like `claude login`.
    pub fn spawn_command(
        &mut self,
        session_id: Uuid,
        program: &str,
        args: &[&str],
        app_handle: AppHandle,
    ) -> Result<(), String> {
        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("failed to open pty: {}", e))?;

        let mut cmd = CommandBuilder::new(program);
        for arg in args {
            cmd.arg(*arg);
        }
        cmd.env_remove("CLAUDECODE");

        let _child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("failed to spawn {}: {}", program, e))?;

        drop(pair.slave);

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("failed to get pty writer: {}", e))?;

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("failed to get pty reader: {}", e))?;

        let alive = Arc::new(Mutex::new(true));
        let alive_clone = Arc::clone(&alive);

        let output_event = format!("pty-output:{}", session_id);
        let status_event = format!("session-status-changed:{}", session_id);

        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        if let Ok(mut a) = alive_clone.lock() {
                            *a = false;
                        }
                        let _ = app_handle.emit(&status_event, "idle");
                        break;
                    }
                    Ok(n) => {
                        let encoded =
                            base64::engine::general_purpose::STANDARD.encode(&buf[..n]);
                        let _ = app_handle.emit(&output_event, encoded);
                    }
                    Err(_) => {
                        if let Ok(mut a) = alive_clone.lock() {
                            *a = false;
                        }
                        let _ = app_handle.emit(&status_event, "idle");
                        break;
                    }
                }
            }
        });

        let session = PtySession {
            master: pair.master,
            writer,
            alive,
            tmux_session: false,
        };

        self.sessions.insert(session_id, session);
        Ok(())
    }

    pub fn write_to_session(&mut self, session_id: Uuid, data: &[u8]) -> Result<(), String> {
        let session = self
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| format!("session not found: {}", session_id))?;

        session
            .writer
            .write_all(data)
            .map_err(|e| format!("failed to write to pty: {}", e))?;

        session
            .writer
            .flush()
            .map_err(|e| format!("failed to flush pty writer: {}", e))?;

        Ok(())
    }

    /// Send raw bytes that must bypass tmux's outer terminal parser.
    /// For tmux sessions, uses `tmux send-keys -H`; for direct sessions,
    /// writes to the PTY like normal.
    pub fn send_raw_to_session(&mut self, session_id: Uuid, data: &[u8]) -> Result<(), String> {
        let session = self
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| format!("session not found: {}", session_id))?;

        if session.tmux_session {
            TmuxManager::send_keys_hex(session_id, data)
        } else {
            session
                .writer
                .write_all(data)
                .map_err(|e| format!("failed to write to pty: {}", e))?;
            session
                .writer
                .flush()
                .map_err(|e| format!("failed to flush pty writer: {}", e))?;
            Ok(())
        }
    }

    pub fn resize_session(&self, session_id: Uuid, rows: u16, cols: u16) -> Result<(), String> {
        let session = self
            .sessions
            .get(&session_id)
            .ok_or_else(|| format!("session not found: {}", session_id))?;

        // Resize via tmux so the claude process sees the new size
        if session.tmux_session {
            let _ = TmuxManager::resize_session(session_id, cols, rows);
        }

        // Also resize the local PTY
        session
            .master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("failed to resize pty: {}", e))
    }

    pub fn is_alive(&self, session_id: Uuid) -> bool {
        self.sessions
            .get(&session_id)
            .and_then(|s| s.alive.lock().ok())
            .map(|a| *a)
            .unwrap_or(false)
    }

    /// Close a session. For tmux-backed sessions, also kills the tmux session.
    pub fn close_session(&mut self, session_id: Uuid) -> Result<(), String> {
        let session = self.sessions.remove(&session_id);

        if let Some(s) = &session {
            if s.tmux_session {
                let _ = TmuxManager::kill_session(session_id);
            }
        } else {
            // No local PTY (e.g., app wasn't attached), still try to kill tmux
            let _ = TmuxManager::kill_session(session_id);
        }

        Ok(())
    }

    /// Get all tracked session IDs.
    pub fn session_ids(&self) -> Vec<Uuid> {
        self.sessions.keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_manager_is_empty_and_is_alive_returns_false() {
        let manager = PtyManager::new();
        let random_id = Uuid::new_v4();
        assert!(!manager.is_alive(random_id));
    }

    #[test]
    fn test_write_to_invalid_session_returns_error() {
        let mut manager = PtyManager::new();
        let invalid_id = Uuid::new_v4();
        let result = manager.write_to_session(invalid_id, b"hello");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("session not found"));
    }

    #[test]
    fn test_resize_invalid_session_returns_error() {
        let manager = PtyManager::new();
        let invalid_id = Uuid::new_v4();
        let result = manager.resize_session(invalid_id, 24, 80);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("session not found"));
    }

    #[test]
    fn test_close_nonexistent_session_is_ok() {
        let mut manager = PtyManager::new();
        let invalid_id = Uuid::new_v4();
        // close_session is now idempotent (tries to kill tmux too)
        let result = manager.close_session(invalid_id);
        assert!(result.is_ok());
    }
}
