use base64::Engine;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

pub struct PtySession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    alive: Arc<Mutex<bool>>,
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

    pub fn spawn_session(
        &mut self,
        session_id: Uuid,
        working_dir: &str,
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

        let mut cmd = CommandBuilder::new("claude");
        cmd.cwd(working_dir);
        // Prevent Claude from detecting a nested session
        cmd.env_remove("CLAUDECODE");

        // Spawn command on the slave end
        let _child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("failed to spawn claude: {}", e))?;

        // Drop the slave — the master side keeps the PTY alive
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

        // Spawn a reader thread that forwards PTY output to the frontend via Tauri events
        let output_event = format!("pty-output:{}", session_id);
        let status_event = format!("session-status-changed:{}", session_id);

        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        // EOF — process exited
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
                        // Read error — treat as EOF
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
        };

        self.sessions.insert(session_id, session);
        Ok(())
    }

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
        // Prevent Claude from detecting a nested session
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

    pub fn resize_session(&self, session_id: Uuid, rows: u16, cols: u16) -> Result<(), String> {
        let session = self
            .sessions
            .get(&session_id)
            .ok_or_else(|| format!("session not found: {}", session_id))?;

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

    pub fn close_session(&mut self, session_id: Uuid) -> Result<(), String> {
        self.sessions
            .remove(&session_id)
            .ok_or_else(|| format!("session not found: {}", session_id))?;

        // Dropping the PtySession (master + writer) kills the child process
        Ok(())
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
    fn test_close_invalid_session_returns_error() {
        let mut manager = PtyManager::new();
        let invalid_id = Uuid::new_v4();
        let result = manager.close_session(invalid_id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("session not found"));
    }
}
