use base64::Engine;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use uuid::Uuid;

use crate::emitter::EventEmitter;

use crate::tmux::TmuxManager;

fn build_tmux_attach_command(tmux_bin: &str, tmux_name: &str) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(tmux_bin);
    cmd.arg("attach-session");
    cmd.arg("-t");
    cmd.arg(tmux_name);
    cmd
}

pub struct PtySession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    alive: Arc<Mutex<bool>>,
    tmux_session: bool,
}

pub struct PtyManager {
    pub(crate) sessions: HashMap<Uuid, PtySession>,
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
        emitter: Arc<dyn EventEmitter>,
        continue_session: bool,
        initial_prompt: Option<&str>,
        rows: u16,
        cols: u16,
    ) -> Result<(), String> {
        // Skip if already connected
        if self.sessions.contains_key(&session_id) {
            return Ok(());
        }

        let command = match kind {
            "codex" => "codex",
            _ => "claude",
        };
        if TmuxManager::is_available() {
            // Create tmux session if it doesn't already exist
            if !TmuxManager::has_session(session_id) {
                TmuxManager::create_session(
                    session_id,
                    working_dir,
                    command,
                    continue_session,
                    initial_prompt,
                )?;
            }
            // Pre-resize tmux to the target size so attaching doesn't cause
            // an intermediate resize (which would make claude re-render and
            // produce extra newlines).
            let _ = TmuxManager::resize_session(session_id, cols, rows);
            // Attach to the tmux session via a local PTY
            self.attach_tmux_session(session_id, emitter)
        } else {
            // No tmux — spawn the command directly in a PTY
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
    }

    /// Spawn a command directly in a local PTY without tmux.
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
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("failed to open pty: {}", e))?;

        let mut cmd = CommandBuilder::new(command);
        cmd.cwd(working_dir);
        cmd.env_remove("CLAUDECODE");
        cmd.env("THE_CONTROLLER_SESSION_ID", session_id.to_string());
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

        let session = PtySession {
            master: pair.master,
            writer,
            child,
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
        emitter: Arc<dyn EventEmitter>,
    ) -> Result<(), String> {
        let tmux_name = TmuxManager::session_name(session_id);

        // Use the tmux session's current dimensions so the attach doesn't
        // force a resize (which causes TUI glitches like garbled input).
        let (cols, rows) = TmuxManager::session_size(session_id).unwrap_or((80, 24));

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("failed to open pty: {}", e))?;

        let tmux_bin =
            TmuxManager::tmux_binary().ok_or_else(|| "tmux binary not found".to_string())?;
        let cmd = build_tmux_attach_command(&tmux_bin, &tmux_name);

        let child = pair
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

        let session = PtySession {
            master: pair.master,
            writer,
            child,
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

        let session = PtySession {
            master: pair.master,
            writer,
            child,
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
        let mut session = self.sessions.remove(&session_id);

        if let Some(s) = session.as_mut() {
            if !matches!(s.child.try_wait(), Ok(Some(_))) {
                let _ = s.child.kill();
                let _ = s.child.wait();
            }
        }

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
    use crate::emitter::NoopEmitter;
    use crate::tmux::set_test_tmux_binary;
    use std::ffi::OsStr;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    fn make_temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("the-controller-{}-{}", name, Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_fake_tmux(dir: &Path, log_path: &Path) -> PathBuf {
        let tmux_path = dir.join("fake-tmux");
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$1\" >> \"{}\"\nif [ \"$1\" = \"display-message\" ]; then\n  printf '80 24\\n'\nfi\nexit 0\n",
            log_path.display()
        );
        fs::write(&tmux_path, script).unwrap();
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&tmux_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&tmux_path, perms).unwrap();
        }
        tmux_path
    }

    fn wait_for_log_entry(log_path: &Path, needle: &str) -> bool {
        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline {
            let log = fs::read_to_string(log_path).unwrap_or_default();
            if log.contains(needle) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        false
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
        // close_session is now idempotent (tries to kill tmux too)
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

    #[test]
    fn test_build_tmux_attach_command_uses_resolved_tmux_binary() {
        let cmd = build_tmux_attach_command(
            "/usr/local/bin/tmux",
            "ctrl-550e8400-e29b-41d4-a716-446655440000",
        );

        assert_eq!(
            cmd.get_argv().first().map(|arg| arg.as_os_str()),
            Some(OsStr::new("/usr/local/bin/tmux"))
        );
    }

    #[test]
    fn test_attach_tmux_session_uses_resolved_tmux_binary() {
        let temp_dir = make_temp_dir("tmux-attach");
        let log_path = temp_dir.join("tmux.log");
        let fake_tmux = write_fake_tmux(&temp_dir, &log_path);
        let _tmux_guard = set_test_tmux_binary(Some(fake_tmux.to_str().unwrap()));

        let mut manager = PtyManager::new();
        let session_id = Uuid::new_v4();

        manager
            .attach_tmux_session(session_id, NoopEmitter::new())
            .expect("attach should use the resolved tmux binary");

        assert!(wait_for_log_entry(&log_path, "display-message"));
        assert!(wait_for_log_entry(&log_path, "attach-session"));

        let _ = manager.close_session(session_id);
        let _ = fs::remove_dir_all(&temp_dir);
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
