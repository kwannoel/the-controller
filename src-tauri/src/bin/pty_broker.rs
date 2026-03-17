use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::io::{Read as _, Write};
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::sync::Arc;
use the_controller_lib::broker_protocol::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, Mutex, Notify};
use uuid::Uuid;

const RING_BUFFER_SIZE: usize = 64 * 1024;

struct RingBuffer {
    buf: Vec<u8>,
    pos: usize,
    len: usize,
}

impl RingBuffer {
    fn new() -> Self {
        Self {
            buf: vec![0u8; RING_BUFFER_SIZE],
            pos: 0,
            len: 0,
        }
    }

    fn write(&mut self, data: &[u8]) {
        for &byte in data {
            self.buf[self.pos] = byte;
            self.pos = (self.pos + 1) % RING_BUFFER_SIZE;
            if self.len < RING_BUFFER_SIZE {
                self.len += 1;
            }
        }
    }

    /// Return the buffered data in order.
    fn snapshot(&self) -> Vec<u8> {
        if self.len < RING_BUFFER_SIZE {
            self.buf[..self.len].to_vec()
        } else {
            let mut out = Vec::with_capacity(RING_BUFFER_SIZE);
            out.extend_from_slice(&self.buf[self.pos..]);
            out.extend_from_slice(&self.buf[..self.pos]);
            out
        }
    }
}

#[allow(dead_code)]
struct BrokerSession {
    id: Uuid,
    alive: Arc<Mutex<bool>>,
    ring: Arc<Mutex<RingBuffer>>,
    /// Senders for connected data-socket clients.
    clients: Arc<Mutex<ClientSenders>>,
    pty_writer: Arc<std::sync::Mutex<Box<dyn Write + Send>>>,
    _master: Box<dyn MasterPty + Send>,
}

type ClientSenders = Vec<mpsc::UnboundedSender<Vec<u8>>>;

struct Broker {
    socket_dir: PathBuf,
    sessions: Arc<Mutex<HashMap<Uuid, BrokerSession>>>,
    activity: Arc<Notify>,
}

impl Broker {
    fn new(socket_dir: PathBuf) -> Self {
        Self {
            socket_dir,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            activity: Arc::new(Notify::new()),
        }
    }

    fn control_socket_path(&self) -> PathBuf {
        self.socket_dir.join("pty-broker.sock")
    }

    fn data_socket_path(&self, session_id: Uuid) -> PathBuf {
        self.socket_dir.join(format!("pty-{}.sock", session_id))
    }

    fn pid_file_path(&self) -> PathBuf {
        self.socket_dir.join("pty-broker.pid")
    }

    fn lock_file_path(&self) -> PathBuf {
        self.socket_dir.join("pty-broker.lock")
    }

    async fn handle_spawn(&self, req: SpawnRequest) -> Response {
        let session_id = req.session_id;

        // Check if session already exists
        {
            let sessions = self.sessions.lock().await;
            if sessions.contains_key(&session_id) {
                return Response::Ok(OkResponse { session_id });
            }
        }

        let pty_system = native_pty_system();
        let pair = match pty_system.openpty(PtySize {
            rows: req.rows,
            cols: req.cols,
            pixel_width: 0,
            pixel_height: 0,
        }) {
            Ok(pair) => pair,
            Err(e) => {
                return Response::Error(ErrorResponse {
                    message: format!("failed to open pty: {}", e),
                })
            }
        };

        let mut cmd = CommandBuilder::new(&req.cmd);
        cmd.cwd(&req.cwd);
        for arg in &req.args {
            cmd.arg(arg);
        }
        // Clear inherited env vars and set only what the client sent
        cmd.env_clear();
        for (key, val) in &req.env {
            cmd.env(key, val);
        }

        let child = match pair.slave.spawn_command(cmd) {
            Ok(child) => child,
            Err(e) => {
                return Response::Error(ErrorResponse {
                    message: format!("failed to spawn {}: {}", req.cmd, e),
                })
            }
        };

        drop(pair.slave);

        let writer = match pair.master.take_writer() {
            Ok(w) => w,
            Err(e) => {
                return Response::Error(ErrorResponse {
                    message: format!("failed to get pty writer: {}", e),
                })
            }
        };

        let mut reader = match pair.master.try_clone_reader() {
            Ok(r) => r,
            Err(e) => {
                return Response::Error(ErrorResponse {
                    message: format!("failed to get pty reader: {}", e),
                })
            }
        };

        let alive = Arc::new(Mutex::new(true));
        let ring = Arc::new(Mutex::new(RingBuffer::new()));
        let clients: Arc<Mutex<ClientSenders>> = Arc::new(Mutex::new(Vec::new()));

        // PTY reader task: reads from PTY, writes to ring buffer + all clients
        let alive_clone = Arc::clone(&alive);
        let ring_clone = Arc::clone(&ring);
        let clients_clone = Arc::clone(&clients);
        let activity = Arc::clone(&self.activity);
        let rt_handle = tokio::runtime::Handle::current();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => {
                        rt_handle.block_on(async {
                            *alive_clone.lock().await = false;
                        });
                        activity.notify_one();
                        break;
                    }
                    Ok(n) => {
                        let data = buf[..n].to_vec();
                        rt_handle.block_on(async {
                            ring_clone.lock().await.write(&data);
                            let mut clients = clients_clone.lock().await;
                            clients.retain(|tx| tx.send(data.clone()).is_ok());
                        });
                    }
                }
            }
        });

        // Wait for child exit in background and clean up session resources
        let alive_child = Arc::clone(&alive);
        let activity_child = Arc::clone(&self.activity);
        let sessions_child = Arc::clone(&self.sessions);
        let data_path_child = self.data_socket_path(session_id);
        tokio::task::spawn_blocking(move || {
            let mut child = child;
            let _ = child.wait();
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                *alive_child.lock().await = false;
                // Clean up: remove session from HashMap and delete the data socket file,
                // mirroring the cleanup done in handle_kill.
                sessions_child.lock().await.remove(&session_id);
                let _ = std::fs::remove_file(&data_path_child);
            });
            activity_child.notify_one();
        });

        // Set up data socket for this session
        let data_path = self.data_socket_path(session_id);
        let _ = std::fs::remove_file(&data_path);
        let data_listener = match UnixListener::bind(&data_path) {
            Ok(l) => l,
            Err(e) => {
                return Response::Error(ErrorResponse {
                    message: format!("failed to bind data socket: {}", e),
                })
            }
        };

        let ring_for_data = Arc::clone(&ring);
        let clients_for_data = Arc::clone(&clients);
        let pty_writer_arc = Arc::new(std::sync::Mutex::new(writer));
        let pty_writer_for_data = Arc::clone(&pty_writer_arc);
        let activity_data = Arc::clone(&self.activity);
        let sessions_for_data = Arc::clone(&self.sessions);

        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = data_listener.accept().await else {
                    break;
                };
                // Stop accepting if the session has been removed (natural exit or kill)
                if !sessions_for_data.lock().await.contains_key(&session_id) {
                    break;
                }
                activity_data.notify_one();
                let ring = Arc::clone(&ring_for_data);
                let clients = Arc::clone(&clients_for_data);
                let pty_writer = Arc::clone(&pty_writer_for_data);
                tokio::spawn(async move {
                    handle_data_client(stream, ring, clients, pty_writer).await;
                });
            }
        });

        let session = BrokerSession {
            id: session_id,
            alive,
            ring,
            clients,
            pty_writer: pty_writer_arc,
            _master: pair.master,
        };

        self.sessions.lock().await.insert(session_id, session);
        self.activity.notify_one();

        Response::Ok(OkResponse { session_id })
    }
    // PLACEHOLDER_BROKER_METHODS

    async fn handle_kill(&self, req: KillRequest) -> Response {
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.remove(&req.session_id) {
            // Drop the master PTY to send SIGHUP to the child
            drop(session._master);
            // Clean up data socket
            let data_path = self.data_socket_path(req.session_id);
            let _ = std::fs::remove_file(&data_path);
            self.activity.notify_one();
            Response::Ok(OkResponse {
                session_id: req.session_id,
            })
        } else {
            Response::Error(ErrorResponse {
                message: format!("session not found: {}", req.session_id),
            })
        }
    }

    async fn handle_resize(&self, req: ResizeRequest) -> Response {
        let sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get(&req.session_id) {
            match session._master.resize(PtySize {
                rows: req.rows,
                cols: req.cols,
                pixel_width: 0,
                pixel_height: 0,
            }) {
                Ok(()) => Response::Ok(OkResponse {
                    session_id: req.session_id,
                }),
                Err(e) => Response::Error(ErrorResponse {
                    message: format!("resize failed: {}", e),
                }),
            }
        } else {
            Response::Error(ErrorResponse {
                message: format!("session not found: {}", req.session_id),
            })
        }
    }

    async fn handle_list(&self) -> Response {
        let entries: Vec<(Uuid, Arc<Mutex<bool>>)> = {
            let sessions = self.sessions.lock().await;
            sessions
                .values()
                .map(|s| (s.id, Arc::clone(&s.alive)))
                .collect()
        };
        let mut infos = Vec::new();
        for (id, alive_lock) in entries {
            let alive = *alive_lock.lock().await;
            infos.push(SessionInfo { id, alive });
        }
        Response::List(ListResponse { sessions: infos })
    }

    async fn handle_has_session(&self, req: HasSessionRequest) -> Response {
        let alive_lock = {
            let sessions = self.sessions.lock().await;
            sessions.get(&req.session_id).map(|s| Arc::clone(&s.alive))
        };
        if let Some(alive_lock) = alive_lock {
            let alive = *alive_lock.lock().await;
            Response::HasSession(HasSessionResponse { alive })
        } else {
            Response::HasSession(HasSessionResponse { alive: false })
        }
    }

    async fn handle_request(&self, req: Request) -> Option<Response> {
        match req {
            Request::Spawn(r) => Some(self.handle_spawn(r).await),
            Request::Kill(r) => Some(self.handle_kill(r).await),
            Request::Resize(r) => Some(self.handle_resize(r).await),
            Request::List => Some(self.handle_list().await),
            Request::HasSession(r) => Some(self.handle_has_session(r).await),
            Request::Shutdown => None, // Signal to shut down
        }
    }

    fn cleanup(&self) {
        let _ = std::fs::remove_file(self.control_socket_path());
        let _ = std::fs::remove_file(self.pid_file_path());
        let _ = std::fs::remove_file(self.lock_file_path());
    }
}

async fn handle_data_client(
    stream: UnixStream,
    ring: Arc<Mutex<RingBuffer>>,
    clients: Arc<Mutex<Vec<mpsc::UnboundedSender<Vec<u8>>>>>,
    pty_writer: Arc<std::sync::Mutex<Box<dyn Write + Send>>>,
) {
    let (mut read_half, mut write_half) = stream.into_split();

    // Dump ring buffer snapshot first
    let snapshot = ring.lock().await.snapshot();
    if !snapshot.is_empty() && write_half.write_all(&snapshot).await.is_err() {
        return;
    }

    // Register as a live client
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
    clients.lock().await.push(tx);

    // Forward PTY output to this client
    let write_task = tokio::spawn(async move {
        while let Some(data) = rx.recv().await {
            if write_half.write_all(&data).await.is_err() {
                break;
            }
        }
    });

    // Forward client input to PTY stdin
    let read_task = tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        loop {
            match read_half.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let data = buf[..n].to_vec();
                    let pty_writer = Arc::clone(&pty_writer);
                    let ok = tokio::task::spawn_blocking(move || {
                        let mut writer = pty_writer.lock().unwrap();
                        if writer.write_all(&data).is_err() {
                            return false;
                        }
                        let _ = writer.flush();
                        true
                    })
                    .await
                    .unwrap_or(false);
                    if !ok {
                        break;
                    }
                }
            }
        }
    });

    let _ = tokio::join!(write_task, read_task);
}

fn daemonize() -> Result<(), String> {
    unsafe {
        // First fork
        let pid = libc::fork();
        if pid < 0 {
            return Err("first fork failed".to_string());
        }
        if pid > 0 {
            // Parent exits
            libc::_exit(0);
        }

        // Create new session
        if libc::setsid() < 0 {
            return Err("setsid failed".to_string());
        }

        // Second fork
        let pid = libc::fork();
        if pid < 0 {
            return Err("second fork failed".to_string());
        }
        if pid > 0 {
            // First child exits
            libc::_exit(0);
        }
    }

    // Redirect stdin/stdout/stderr to /dev/null
    let devnull = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")
        .map_err(|e| format!("failed to open /dev/null: {}", e))?;

    use std::os::unix::io::AsRawFd;
    unsafe {
        libc::dup2(devnull.as_raw_fd(), 0);
        libc::dup2(devnull.as_raw_fd(), 1);
        libc::dup2(devnull.as_raw_fd(), 2);
    }

    Ok(())
}
// PLACEHOLDER_MAIN

fn main() {
    let mut socket_dir = PathBuf::from("/tmp/the-controller");
    let mut foreground = false;

    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--build-date" => {
                println!("{}", env!("BUILD_DATE"));
                std::process::exit(0);
            }
            "--socket-dir" => {
                i += 1;
                if i < args.len() {
                    socket_dir = PathBuf::from(&args[i]);
                }
            }
            "--foreground" => {
                foreground = true;
            }
            _ => {}
        }
        i += 1;
    }

    if let Err(e) = std::fs::create_dir_all(&socket_dir) {
        eprintln!("failed to create socket dir: {}", e);
        std::process::exit(1);
    }

    // Daemonize BEFORE creating the tokio runtime — fork invalidates
    // the runtime's thread pool and kqueue/epoll file descriptors.
    if !foreground {
        if let Err(e) = daemonize() {
            eprintln!("failed to daemonize: {}", e);
            std::process::exit(1);
        }
    }

    // Initialize logging AFTER daemonize() — the non-blocking writer spawns a
    // background consumer thread that would be killed by fork().
    let base_dir = dirs::home_dir()
        .map(|h| h.join(".the-controller"))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let _log_guard = the_controller_lib::logging::init_broker_logging(&base_dir, foreground);

    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async_main(socket_dir));
}

async fn async_main(socket_dir: PathBuf) {
    let broker = Arc::new(Broker::new(socket_dir));

    // Acquire exclusive lock file — prevents multiple brokers from running.
    // The fd is held for the broker's entire lifetime; the OS releases it on exit/crash.
    let lock_file = match std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(broker.lock_file_path())
    {
        Ok(f) => f,
        Err(e) => {
            eprintln!("failed to open lock file: {}", e);
            std::process::exit(1);
        }
    };
    let lock_ret = unsafe { libc::flock(lock_file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if lock_ret != 0 {
        // Another broker already holds the lock — exit silently.
        std::process::exit(0);
    }
    // Keep _lock_file alive (held open) for the broker's entire lifetime.
    let _lock_file = lock_file;

    // Write PID file
    let pid = std::process::id();
    if let Err(e) = std::fs::write(broker.pid_file_path(), pid.to_string()) {
        tracing::error!("failed to write pid file: {}", e);
        std::process::exit(1);
    }

    // Clean up stale control socket
    let control_path = broker.control_socket_path();
    let _ = std::fs::remove_file(&control_path);

    let control_listener = match UnixListener::bind(&control_path) {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("failed to bind control socket: {}", e);
            std::process::exit(1);
        }
    };

    // Set up signal handling
    let shutdown = Arc::new(Notify::new());
    let shutdown_signal = Arc::clone(&shutdown);

    tokio::spawn(async move {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate()).expect("failed to register SIGTERM");
        let mut sigint = signal(SignalKind::interrupt()).expect("failed to register SIGINT");
        tokio::select! {
            _ = sigterm.recv() => {},
            _ = sigint.recv() => {},
        }
        shutdown_signal.notify_one();
    });

    // Main accept loop
    let broker_main = Arc::clone(&broker);

    tokio::select! {
        _ = async {
            while let Ok((stream, _)) = control_listener.accept().await {
                broker_main.activity.notify_one();
                let broker = Arc::clone(&broker_main);
                let shutdown = Arc::clone(&shutdown);
                tokio::spawn(async move {
                    handle_control_client(stream, broker, shutdown).await;
                });
            }
        } => {},
        _ = shutdown.notified() => {},
    }

    // Graceful shutdown: kill all sessions, clean up sockets
    let mut sessions = broker.sessions.lock().await;
    let ids: Vec<Uuid> = sessions.keys().copied().collect();
    for id in ids {
        if let Some(session) = sessions.remove(&id) {
            drop(session._master);
            let data_path = broker.data_socket_path(id);
            let _ = std::fs::remove_file(&data_path);
        }
    }
    drop(sessions);
    broker.cleanup();
}

async fn handle_control_client(mut stream: UnixStream, broker: Arc<Broker>, shutdown: Arc<Notify>) {
    loop {
        let req = match recv_request(&mut stream).await {
            Ok(req) => req,
            Err(_) => break,
        };

        if let Request::Shutdown = &req {
            // Notify shutdown FIRST so the main accept loop stops before
            // the client receives the response — prevents new connections
            // from arriving between response and shutdown.
            shutdown.notify_one();
            let resp = Response::Ok(OkResponse {
                session_id: Uuid::nil(),
            });
            let _ = send_response(&mut stream, &resp).await;
            break;
        }

        if let Some(resp) = broker.handle_request(req).await {
            if send_response(&mut stream, &resp).await.is_err() {
                break;
            }
        }
    }
}
