use crate::broker_protocol::*;
use std::io::{self, Read, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

/// Cached staleness check result, keyed by broker PID.
/// Once we verify a broker with a given PID is NOT stale, we skip the
/// expensive `--build-date` subprocess on subsequent calls.
/// If the broker restarts (new PID), the cache miss forces a re-check.
static STALE_CHECK_CACHE: Mutex<Option<(i32, bool)>> = Mutex::new(None);

/// Default socket directory for the PTY broker.
fn default_socket_dir() -> PathBuf {
    PathBuf::from("/tmp/the-controller")
}

/// Synchronous client for communicating with the PTY broker daemon.
/// All control operations use blocking I/O (suitable for use behind std::sync::Mutex).
pub struct BrokerClient {
    socket_dir: PathBuf,
}

impl Default for BrokerClient {
    fn default() -> Self {
        Self::new()
    }
}

impl BrokerClient {
    pub fn new() -> Self {
        Self {
            socket_dir: default_socket_dir(),
        }
    }

    fn control_socket_path(&self) -> PathBuf {
        self.socket_dir.join("pty-broker.sock")
    }

    pub fn data_socket_path(&self, session_id: Uuid) -> PathBuf {
        self.socket_dir.join(format!("pty-{}.sock", session_id))
    }

    fn pid_file_path(&self) -> PathBuf {
        self.socket_dir.join("pty-broker.pid")
    }

    fn lock_file_path(&self) -> PathBuf {
        self.socket_dir.join("pty-broker.lock")
    }

    fn broker_binary_path() -> Option<PathBuf> {
        crate::cli_install::controller_bin_dir().map(|d| d.join("pty-broker"))
    }

    /// Check if the broker is reachable (non-blocking probe).
    pub fn is_available(&self) -> bool {
        UnixStream::connect(self.control_socket_path()).is_ok()
    }

    /// Try to ensure the broker is running, spawning it if the binary exists.
    /// Returns true if the broker is reachable after this call.
    pub fn try_ensure_running(&self) -> bool {
        self.connect_control().is_ok()
    }

    /// Connect to the control socket, spawning the broker if needed.
    /// If a running broker has a stale build date, shuts it down and respawns.
    fn connect_control(&self) -> io::Result<UnixStream> {
        let path = self.control_socket_path();

        // Try connecting directly first
        if let Ok(stream) = UnixStream::connect(&path) {
            // Check if the running broker is stale
            if self.is_broker_stale() {
                let old_pid = self.read_pid();
                // Shut down the stale broker (best-effort)
                let _ = self.send_shutdown_to(&stream);
                drop(stream);
                // Wait for the old process to actually exit
                if let Some(pid) = old_pid {
                    if !self.wait_for_pid_exit(pid, std::time::Duration::from_secs(3)) {
                        // Escalate to SIGKILL if graceful shutdown didn't work
                        unsafe {
                            libc::kill(pid, libc::SIGKILL);
                        }
                        self.wait_for_pid_exit(pid, std::time::Duration::from_millis(500));
                    }
                }
                self.cleanup_stale_pid();
            } else {
                return Ok(stream);
            }
        }

        // Check for stale PID file
        self.cleanup_stale_pid();

        // Try to spawn the broker
        self.spawn_broker()?;

        // Retry connection with backoff
        for i in 0..20 {
            std::thread::sleep(std::time::Duration::from_millis(50 * (i + 1)));
            if let Ok(stream) = UnixStream::connect(&path) {
                return Ok(stream);
            }
        }

        Err(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            "failed to connect to broker after spawning",
        ))
    }

    /// Read the PID from the PID file, if it exists and is valid.
    fn read_pid(&self) -> Option<i32> {
        let contents = std::fs::read_to_string(self.pid_file_path()).ok()?;
        contents.trim().parse::<i32>().ok()
    }

    /// Poll until a process exits or the timeout expires. Returns true if the process exited.
    fn wait_for_pid_exit(&self, pid: i32, timeout: std::time::Duration) -> bool {
        let start = std::time::Instant::now();
        let poll_interval = std::time::Duration::from_millis(50);
        while start.elapsed() < timeout {
            let alive = unsafe { libc::kill(pid, 0) } == 0;
            if !alive {
                return true;
            }
            std::thread::sleep(poll_interval);
        }
        false
    }

    /// Check if the PID file points to a dead process and clean up if so.
    fn cleanup_stale_pid(&self) {
        let pid_path = self.pid_file_path();
        if let Ok(contents) = std::fs::read_to_string(&pid_path) {
            if let Ok(pid) = contents.trim().parse::<i32>() {
                let alive = unsafe { libc::kill(pid, 0) } == 0;
                if !alive {
                    let _ = std::fs::remove_file(&pid_path);
                    let _ = std::fs::remove_file(self.control_socket_path());
                }
            }
        }
    }

    /// Check if the installed broker binary has a different build date than this app.
    /// Result is cached per broker PID to avoid forking a subprocess on every RPC call.
    fn is_broker_stale(&self) -> bool {
        let current_pid = self.read_pid();

        // Check cache: if we already verified this PID, return the cached result.
        if let Some(pid) = current_pid {
            if let Ok(cache) = STALE_CHECK_CACHE.lock() {
                if let Some((cached_pid, cached_stale)) = *cache {
                    if cached_pid == pid {
                        return cached_stale;
                    }
                }
            }
        }

        let stale = self.check_broker_stale();

        // Cache the result if we have a valid PID.
        if let Some(pid) = current_pid {
            if let Ok(mut cache) = STALE_CHECK_CACHE.lock() {
                *cache = Some((pid, stale));
            }
        }

        stale
    }

    /// Perform the actual staleness check by spawning the broker binary.
    fn check_broker_stale(&self) -> bool {
        let Some(binary) = Self::broker_binary_path() else {
            return false;
        };
        if !binary.exists() {
            return false;
        }
        let Ok(output) = std::process::Command::new(&binary)
            .arg("--build-date")
            .output()
        else {
            return false;
        };
        if !output.status.success() {
            // Old binary without --build-date support — treat as stale
            return true;
        }
        let broker_date = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let our_date = env!("BUILD_DATE");
        broker_date != our_date
    }

    /// Send a Shutdown request on an existing stream (best-effort).
    fn send_shutdown_to(&self, stream: &UnixStream) -> io::Result<()> {
        let mut stream = stream.try_clone()?;
        let frame = encode_request(&Request::Shutdown)?;
        stream.write_all(&frame)?;
        Ok(())
    }

    /// Spawn the broker binary as a daemon.
    /// Uses a lock file to prevent multiple concurrent spawns.
    fn spawn_broker(&self) -> io::Result<()> {
        let binary = Self::broker_binary_path().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "pty-broker binary not found")
        })?;

        if !binary.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("pty-broker binary not found at {}", binary.display()),
            ));
        }

        let _ = std::fs::create_dir_all(&self.socket_dir);

        // Try to acquire the lock file non-blocking. If another broker (or spawner)
        // already holds it, skip spawning — the retry loop in connect_control()
        // will wait for the existing broker to become ready.
        let lock_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(self.lock_file_path())?;
        let lock_ret = unsafe { libc::flock(lock_file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if lock_ret != 0 {
            // Lock is held — another broker is already starting or running.
            // Drop the fd and let the connect retry loop handle it.
            return Ok(());
        }
        // We hold the lock briefly to prevent concurrent spawns.
        // The broker process itself will acquire the lock on startup,
        // so we release ours immediately after spawning.

        std::process::Command::new(&binary)
            .arg("--socket-dir")
            .arg(&self.socket_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;

        // Drop the lock file fd — the broker will acquire its own lock.
        drop(lock_file);

        Ok(())
    }

    /// Send a framed request and read a framed response (blocking).
    fn request(&self, req: &Request) -> Result<Response, String> {
        let mut stream = self
            .connect_control()
            .map_err(|e| format!("broker connection failed: {}", e))?;

        let frame = encode_request(req).map_err(|e| format!("failed to encode request: {}", e))?;
        stream
            .write_all(&frame)
            .map_err(|e| format!("failed to send request: {}", e))?;

        // Read response header
        let mut header = [0u8; 5];
        stream
            .read_exact(&mut header)
            .map_err(|e| format!("failed to read response header: {}", e))?;
        let len = u32::from_be_bytes([header[1], header[2], header[3], header[4]]) as usize;
        if len > MAX_MESSAGE_SIZE {
            return Err(format!(
                "message size {} exceeds maximum {}",
                len, MAX_MESSAGE_SIZE
            ));
        }
        let mut payload = vec![0u8; len];
        if len > 0 {
            stream
                .read_exact(&mut payload)
                .map_err(|e| format!("failed to read response payload: {}", e))?;
        }
        let mut full = Vec::with_capacity(5 + len);
        full.extend_from_slice(&header);
        full.extend_from_slice(&payload);
        match decode_response(&full) {
            Ok(Some((resp, _))) => Ok(resp),
            Ok(None) => Err("incomplete response".to_string()),
            Err(e) => Err(format!("failed to decode response: {}", e)),
        }
    }

    /// Spawn a new session in the broker.
    pub fn spawn(&self, req: SpawnRequest) -> Result<Uuid, String> {
        match self.request(&Request::Spawn(req))? {
            Response::Ok(r) => Ok(r.session_id),
            Response::Error(e) => Err(e.message),
            other => Err(format!("unexpected response: {:?}", other)),
        }
    }

    /// Kill a session.
    pub fn kill(&self, session_id: Uuid) -> Result<(), String> {
        match self.request(&Request::Kill(KillRequest { session_id }))? {
            Response::Ok(_) => Ok(()),
            Response::Error(e) => Err(e.message),
            other => Err(format!("unexpected response: {:?}", other)),
        }
    }

    /// Resize a session.
    pub fn resize(&self, session_id: Uuid, rows: u16, cols: u16) -> Result<(), String> {
        match self.request(&Request::Resize(ResizeRequest {
            session_id,
            rows,
            cols,
        }))? {
            Response::Ok(_) => Ok(()),
            Response::Error(e) => Err(e.message),
            other => Err(format!("unexpected response: {:?}", other)),
        }
    }

    /// Check if a session exists and is alive.
    pub fn has_session(&self, session_id: Uuid) -> bool {
        match self.request(&Request::HasSession(HasSessionRequest { session_id })) {
            Ok(Response::HasSession(r)) => r.alive,
            _ => false,
        }
    }

    /// Send shutdown to the broker.
    pub fn shutdown(&self) -> Result<(), String> {
        match self.request(&Request::Shutdown)? {
            Response::Ok(_) => Ok(()),
            Response::Error(e) => Err(e.message),
            other => Err(format!("unexpected response: {:?}", other)),
        }
    }

    /// Connect to a session's data socket for raw I/O (blocking).
    pub fn connect_data(&self, session_id: Uuid) -> io::Result<UnixStream> {
        let path = self.data_socket_path(session_id);
        UnixStream::connect(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_socket_dir_is_tmp() {
        let dir = default_socket_dir();
        assert_eq!(dir, PathBuf::from("/tmp/the-controller"));
    }

    #[test]
    fn control_socket_path_format() {
        let client = BrokerClient::new();
        assert!(client
            .control_socket_path()
            .to_string_lossy()
            .ends_with("pty-broker.sock"));
    }

    #[test]
    fn data_socket_path_format() {
        let client = BrokerClient::new();
        let id = Uuid::nil();
        let path = client.data_socket_path(id);
        assert!(path.to_string_lossy().contains("pty-"));
        assert!(path.to_string_lossy().ends_with(".sock"));
    }
}
