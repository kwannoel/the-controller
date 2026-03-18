use base64::Engine;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use uuid::Uuid;

use crate::broker_client::BrokerClient;
use crate::broker_protocol::SpawnRequest;
use crate::emitter::EventEmitter;

/// A direct PTY session (local process).
pub struct PtySession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    alive: Arc<Mutex<bool>>,
}

/// A broker-backed session (PTY held by the broker daemon).
pub struct BrokerSession {
    writer: std::os::unix::net::UnixStream,
    alive: Arc<Mutex<bool>>,
}

pub enum Session {
    Pty(PtySession),
    Broker(BrokerSession),
}

pub struct PtyManager {
    pub(crate) sessions: HashMap<Uuid, Session>,
    broker: BrokerClient,
}

impl Default for PtyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            broker: BrokerClient::new(),
        }
    }

    /// Spawn a session. Tries broker first (survives dev restarts),
    /// then falls back to a direct PTY.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_session(
        &mut self,
        session_id: Uuid,
        working_dir: &str,
        kind: &str,
        emitter: Arc<dyn EventEmitter>,
        continue_session: bool,
        initial_prompt: Option<&str>,
        rows: u16,
        cols: u16,
    ) -> Result<(), String> {
        // Skip if already connected
        if self.sessions.contains_key(&session_id) {
            tracing::debug!(session_id = %session_id, "session already connected, skipping spawn");
            return Ok(());
        }

        let command = match kind {
            "codex" => "codex",
            "cursor-agent" => "cursor-agent",
            _ => "claude",
        };

        tracing::info!(session_id = %session_id, command = command, "spawning session");

        // Try broker first
        if self.broker.is_available() || self.try_spawn_broker() {
            match self.spawn_broker_session(
                session_id,
                working_dir,
                command,
                emitter.clone(),
                continue_session,
                initial_prompt,
                rows,
                cols,
            ) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    tracing::warn!("broker session failed, falling back to direct PTY: {}", e);
                }
            }
        }

        // Direct PTY
        tracing::debug!(session_id = %session_id, "using direct PTY mode");
        self.spawn_direct_session(
            session_id,
            working_dir,
            command,
            emitter,
            initial_prompt,
            rows,
            cols,
        )
    }

    /// Try to make the broker available (spawn it if binary exists).
    /// Calls connect_control() which handles spawning the daemon and retrying.
    fn try_spawn_broker(&self) -> bool {
        self.broker.try_ensure_running()
    }

    /// Spawn a session via the broker daemon.
    #[allow(clippy::too_many_arguments)]
    fn spawn_broker_session(
        &mut self,
        session_id: Uuid,
        working_dir: &str,
        command: &str,
        emitter: Arc<dyn EventEmitter>,
        continue_session: bool,
        initial_prompt: Option<&str>,
        rows: u16,
        cols: u16,
    ) -> Result<(), String> {
        // Build the env map
        let mut env: HashMap<String, String> = std::env::vars().collect();
        env.remove("CLAUDECODE");
        env.insert(
            "THE_CONTROLLER_SESSION_ID".to_string(),
            session_id.to_string(),
        );
        if let Some(path_val) = crate::cli_install::path_with_controller_bin() {
            env.insert("PATH".to_string(), path_val);
        }

        // Build args
        let args = crate::session_args::build_session_args(
            command,
            session_id,
            continue_session,
            initial_prompt,
        );

        // Check if session already exists in broker
        let needs_spawn = !self.broker.has_session(session_id);

        if needs_spawn {
            tracing::debug!(session_id = %session_id, command = command, "spawning new broker session");
            self.broker.spawn(SpawnRequest {
                session_id,
                cmd: command.to_string(),
                args,
                cwd: working_dir.to_string(),
                env,
                rows,
                cols,
            })?;
        } else {
            // Session exists, just resize
            tracing::debug!(session_id = %session_id, "broker session already exists, resizing");
            let _ = self.broker.resize(session_id, rows, cols);
        }

        // Connect to data socket
        tracing::debug!(session_id = %session_id, "connecting to broker data socket");
        let data_stream = match self.broker.connect_data(session_id) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(session_id = %session_id, error = %e, "failed to connect to broker data socket");
                // Kill the broker session to avoid an orphan
                if needs_spawn {
                    let _ = self.broker.kill(session_id);
                }
                return Err(format!("failed to connect to data socket: {}", e));
            }
        };

        let writer = data_stream
            .try_clone()
            .map_err(|e| format!("failed to clone data socket: {}", e))?;

        let alive = Arc::new(Mutex::new(true));
        let alive_clone = Arc::clone(&alive);

        let output_event = format!("pty-output:{}", session_id);
        let status_event = format!("session-status-changed:{}", session_id);

        // Reader thread: data socket → base64 → emit pty-output
        let mut reader = data_stream;
        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => {
                        if let Ok(mut a) = alive_clone.lock() {
                            *a = false;
                        }
                        let _ = emitter.emit(&status_event, "idle");
                        break;
                    }
                    Ok(n) => {
                        let encoded = base64::engine::general_purpose::STANDARD.encode(&buf[..n]);
                        let _ = emitter.emit(&output_event, &encoded);
                    }
                }
            }
        });

        self.sessions
            .insert(session_id, Session::Broker(BrokerSession { writer, alive }));
        Ok(())
    }

    /// Spawn a command directly in a local PTY.
    #[allow(clippy::too_many_arguments)]
    fn spawn_direct_session(
        &mut self,
        session_id: Uuid,
        working_dir: &str,
        command: &str,
        emitter: Arc<dyn EventEmitter>,
        initial_prompt: Option<&str>,
        rows: u16,
        cols: u16,
    ) -> Result<(), String> {
        tracing::debug!(session_id = %session_id, command = command, "spawning direct PTY session");
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| {
                tracing::error!(session_id = %session_id, error = %e, "failed to open pty");
                format!("failed to open pty: {}", e)
            })?;

        let mut cmd = CommandBuilder::new(command);
        cmd.cwd(working_dir);
        cmd.env_remove("CLAUDECODE");
        cmd.env("THE_CONTROLLER_SESSION_ID", session_id.to_string());
        // Prepend ~/.the-controller/bin to PATH so controller-cli is available
        if let Some(path_val) = crate::cli_install::path_with_controller_bin() {
            cmd.env("PATH", path_val);
        }
        for arg in
            crate::session_args::build_session_args(command, session_id, false, initial_prompt)
        {
            cmd.arg(arg);
        }

        let child = pair
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
                        let _ = emitter.emit(&status_event, "idle");
                        break;
                    }
                    Ok(n) => {
                        let encoded = base64::engine::general_purpose::STANDARD.encode(&buf[..n]);
                        let _ = emitter.emit(&output_event, &encoded);
                    }
                    Err(_) => {
                        if let Ok(mut a) = alive_clone.lock() {
                            *a = false;
                        }
                        let _ = emitter.emit(&status_event, "idle");
                        break;
                    }
                }
            }
        });

        let session = Session::Pty(PtySession {
            master: pair.master,
            writer,
            child,
            alive,
        });

        self.sessions.insert(session_id, session);
        Ok(())
    }

    /// Spawn a direct command. Used for short-lived commands like `claude login`.
    pub fn spawn_command(
        &mut self,
        session_id: Uuid,
        program: &str,
        args: &[&str],
        emitter: Arc<dyn EventEmitter>,
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

        let child = pair
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
                        let _ = emitter.emit(&status_event, "idle");
                        break;
                    }
                    Ok(n) => {
                        let encoded = base64::engine::general_purpose::STANDARD.encode(&buf[..n]);
                        let _ = emitter.emit(&output_event, &encoded);
                    }
                    Err(_) => {
                        if let Ok(mut a) = alive_clone.lock() {
                            *a = false;
                        }
                        let _ = emitter.emit(&status_event, "idle");
                        break;
                    }
                }
            }
        });

        let session = Session::Pty(PtySession {
            master: pair.master,
            writer,
            child,
            alive,
        });

        self.sessions.insert(session_id, session);
        Ok(())
    }

    pub fn write_to_session(&mut self, session_id: Uuid, data: &[u8]) -> Result<(), String> {
        let session = self
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| format!("session not found: {}", session_id))?;

        match session {
            Session::Pty(s) => {
                s.writer
                    .write_all(data)
                    .map_err(|e| format!("failed to write to pty: {}", e))?;
                s.writer
                    .flush()
                    .map_err(|e| format!("failed to flush pty writer: {}", e))?;
            }
            Session::Broker(s) => {
                s.writer
                    .write_all(data)
                    .map_err(|e| format!("failed to write to broker session: {}", e))?;
                s.writer
                    .flush()
                    .map_err(|e| format!("failed to flush broker writer: {}", e))?;
            }
        }
        Ok(())
    }

    /// Send raw bytes directly to the session (single-layer PTY, no tmux workaround needed).
    pub fn send_raw_to_session(&mut self, session_id: Uuid, data: &[u8]) -> Result<(), String> {
        // With the broker or direct PTY, raw writes go straight through.
        self.write_to_session(session_id, data)
    }

    pub fn resize_session(&self, session_id: Uuid, rows: u16, cols: u16) -> Result<(), String> {
        tracing::debug!(session_id = %session_id, rows = rows, cols = cols, "resizing session");
        let session = self
            .sessions
            .get(&session_id)
            .ok_or_else(|| format!("session not found: {}", session_id))?;

        match session {
            Session::Pty(s) => s
                .master
                .resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(|e| format!("failed to resize pty: {}", e)),
            Session::Broker(_) => self.broker.resize(session_id, rows, cols),
        }
    }

    pub fn is_alive(&self, session_id: Uuid) -> bool {
        self.sessions
            .get(&session_id)
            .and_then(|s| match s {
                Session::Pty(s) => s.alive.lock().ok().map(|a| *a),
                Session::Broker(s) => s.alive.lock().ok().map(|a| *a),
            })
            .unwrap_or(false)
    }

    /// Close a session.
    pub fn close_session(&mut self, session_id: Uuid) -> Result<(), String> {
        tracing::info!(session_id = %session_id, "closing session");
        let session = self.sessions.remove(&session_id);

        match session {
            Some(Session::Pty(mut s)) => {
                tracing::debug!(session_id = %session_id, "closing direct PTY session");
                if !matches!(s.child.try_wait(), Ok(Some(_))) {
                    let _ = s.child.kill();
                    let _ = s.child.wait();
                }
            }
            Some(Session::Broker(_)) => {
                tracing::debug!(session_id = %session_id, "closing broker session");
                let _ = self.broker.kill(session_id);
            }
            None => {
                // No local session — try to kill broker session
                let _ = self.broker.kill(session_id);
            }
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
    use crate::emitter::NoopEmitter;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    fn make_temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("the-controller-{}-{}", name, Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[cfg(unix)]
    fn wait_for_pid_file(pid_path: &Path) -> u32 {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if let Ok(pid) = fs::read_to_string(pid_path)
                .ok()
                .and_then(|contents| contents.trim().parse::<u32>().ok())
                .ok_or(())
            {
                return pid;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        panic!("timed out waiting for pid file at {}", pid_path.display());
    }

    #[cfg(unix)]
    fn process_is_alive(pid: u32) -> bool {
        std::process::Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    #[cfg(unix)]
    fn wait_for_process_exit(pid: u32) -> bool {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if !process_is_alive(pid) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        false
    }

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
        // close_session is idempotent
        let result = manager.close_session(invalid_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_session_ids_empty_manager() {
        let manager = PtyManager::new();
        assert!(manager.session_ids().is_empty());
    }

    #[test]
    fn test_send_raw_to_invalid_session_returns_error() {
        let mut manager = PtyManager::new();
        let invalid_id = Uuid::new_v4();
        let result = manager.send_raw_to_session(invalid_id, b"hello");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("session not found"));
    }

    #[cfg(unix)]
    #[test]
    fn test_close_session_kills_direct_child_that_ignores_sighup() {
        let temp_dir = make_temp_dir("close-session-kills-child");
        let script_path = temp_dir.join("ignore-sighup.sh");
        let pid_path = temp_dir.join("child.pid");

        fs::write(
            &script_path,
            format!(
                "echo $$ > '{}'\ntrap '' HUP\nwhile true; do sleep 1; done\n",
                pid_path.display()
            ),
        )
        .unwrap();

        let mut manager = PtyManager::new();
        let session_id = Uuid::new_v4();
        manager
            .spawn_command(
                session_id,
                "/bin/sh",
                &[script_path.to_str().unwrap()],
                NoopEmitter::new(),
            )
            .expect("spawn should succeed");

        let pid = wait_for_pid_file(&pid_path);
        assert!(
            process_is_alive(pid),
            "test child should be running before close_session"
        );

        manager.close_session(session_id).unwrap();

        let exited = wait_for_process_exit(pid);
        if !exited {
            let _ = std::process::Command::new("kill")
                .arg("-9")
                .arg(pid.to_string())
                .status();
        }

        assert!(
            exited,
            "close_session should terminate a PTY child even if it ignores SIGHUP"
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
