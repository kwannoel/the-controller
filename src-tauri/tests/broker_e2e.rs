//! End-to-end tests for the PTY broker daemon.
//!
//! Each test spawns a real `pty-broker` binary in `--foreground` mode with an
//! isolated temp socket directory, then exercises the control + data protocol.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use the_controller_lib::broker_protocol::*;
use uuid::Uuid;

/// Build the pty-broker binary once (relies on cargo test having built it).
fn broker_binary() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_BIN_EXE_pty-broker"));
    if !path.exists() {
        path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("debug")
            .join("pty-broker");
    }
    assert!(path.exists(), "pty-broker binary not found at {:?}", path);
    path
}

/// Short socket dir under /tmp to stay within SUN_LEN (104 bytes on macOS).
fn short_socket_dir() -> PathBuf {
    let id = &Uuid::new_v4().to_string()[..8];
    let dir = PathBuf::from(format!("/tmp/tc-{}", id));
    std::fs::create_dir_all(&dir).expect("create short socket dir");
    dir
}

struct BrokerGuard {
    child: Child,
    socket_dir: PathBuf,
}

impl BrokerGuard {
    fn control_path(&self) -> PathBuf {
        self.socket_dir.join("pty-broker.sock")
    }

    fn data_path(&self, session_id: Uuid) -> PathBuf {
        self.socket_dir.join(format!("pty-{}.sock", session_id))
    }
}

impl Drop for BrokerGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.socket_dir);
    }
}

/// Start a broker in foreground mode with an isolated temp dir.
/// Waits until the control socket is ready.
fn start_broker() -> BrokerGuard {
    let socket_dir = short_socket_dir();
    let child = Command::new(broker_binary())
        .arg("--foreground")
        .arg("--socket-dir")
        .arg(&socket_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn pty-broker");

    let guard = BrokerGuard { child, socket_dir };

    // Wait for control socket to appear
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if guard.control_path().exists() {
            std::thread::sleep(Duration::from_millis(50));
            return guard;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("broker control socket did not appear within 5s");
}

/// Send a request on a control socket and read the response (blocking).
fn control_request(path: &Path, req: &Request) -> Response {
    let mut stream = UnixStream::connect(path).expect("connect to control socket");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();

    let frame = encode_request(req).expect("encode request");
    stream.write_all(&frame).expect("send request");

    let mut header = [0u8; 5];
    stream
        .read_exact(&mut header)
        .expect("read response header");
    let len = u32::from_be_bytes([header[1], header[2], header[3], header[4]]) as usize;
    let mut payload = vec![0u8; len];
    if len > 0 {
        stream
            .read_exact(&mut payload)
            .expect("read response payload");
    }
    let mut full = Vec::with_capacity(5 + len);
    full.extend_from_slice(&header);
    full.extend_from_slice(&payload);
    decode_response(&full)
        .expect("decode response")
        .expect("complete response")
        .0
}

fn make_spawn_request(session_id: Uuid, cmd: &str, args: &[&str]) -> SpawnRequest {
    SpawnRequest {
        session_id,
        cmd: cmd.to_string(),
        args: args.iter().map(|s| s.to_string()).collect(),
        cwd: "/tmp".to_string(),
        env: std::env::vars().collect::<HashMap<String, String>>(),
        rows: 24,
        cols: 80,
    }
}

/// Wait for a data socket to appear, connect, and return the stream.
fn connect_data_socket(path: &Path) -> UnixStream {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !path.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(path.exists(), "data socket should exist at {:?}", path);
    // Small delay to let the broker's accept loop start
    std::thread::sleep(Duration::from_millis(50));
    let stream = UnixStream::connect(path).expect("connect data socket");
    stream
        .set_nonblocking(true)
        .expect("set data socket nonblocking");
    stream
}

/// Read from a non-blocking data socket until `needle` is found or timeout.
fn read_until(stream: &mut UnixStream, needle: &str, timeout: Duration) -> String {
    let mut buf = [0u8; 4096];
    let mut collected = Vec::new();
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                collected.extend_from_slice(&buf[..n]);
                if String::from_utf8_lossy(&collected).contains(needle) {
                    break;
                }
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break,
        }
    }
    String::from_utf8_lossy(&collected).to_string()
}

#[test]
fn test_spawn_and_list() {
    let broker = start_broker();
    let sid = Uuid::new_v4();

    // Spawn a long-running session
    let resp = control_request(
        &broker.control_path(),
        &Request::Spawn(make_spawn_request(sid, "/bin/cat", &[])),
    );
    assert!(
        matches!(resp, Response::Ok(ref r) if r.session_id == sid),
        "spawn should succeed, got: {:?}",
        resp
    );

    // List should show one alive session
    let resp = control_request(&broker.control_path(), &Request::List);
    match resp {
        Response::List(list) => {
            assert_eq!(list.sessions.len(), 1);
            assert_eq!(list.sessions[0].id, sid);
            assert!(list.sessions[0].alive);
        }
        other => panic!("expected List response, got: {:?}", other),
    }
}

#[test]
fn test_has_session() {
    let broker = start_broker();
    let sid = Uuid::new_v4();
    let fake_sid = Uuid::new_v4();

    // Non-existent session
    let resp = control_request(
        &broker.control_path(),
        &Request::HasSession(HasSessionRequest {
            session_id: fake_sid,
        }),
    );
    assert!(matches!(resp, Response::HasSession(r) if !r.alive));

    // Spawn and check
    control_request(
        &broker.control_path(),
        &Request::Spawn(make_spawn_request(sid, "/bin/cat", &[])),
    );
    let resp = control_request(
        &broker.control_path(),
        &Request::HasSession(HasSessionRequest { session_id: sid }),
    );
    assert!(matches!(resp, Response::HasSession(r) if r.alive));
}

#[test]
fn test_data_socket_io_with_cat() {
    let broker = start_broker();
    let sid = Uuid::new_v4();

    control_request(
        &broker.control_path(),
        &Request::Spawn(make_spawn_request(sid, "/bin/cat", &[])),
    );

    let mut data = connect_data_socket(&broker.data_path(sid));
    data.set_nonblocking(false).unwrap();
    data.set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();

    // Write to cat's stdin via data socket
    data.write_all(b"hello broker\n")
        .expect("write to data socket");
    data.flush().unwrap();

    // Switch to nonblocking for reading
    data.set_nonblocking(true).unwrap();
    let output = read_until(&mut data, "hello broker", Duration::from_secs(5));
    assert!(
        output.contains("hello broker"),
        "should see echoed input, got: {:?}",
        output
    );
}

#[test]
fn test_kill_session() {
    let broker = start_broker();
    let sid = Uuid::new_v4();

    control_request(
        &broker.control_path(),
        &Request::Spawn(make_spawn_request(sid, "/bin/cat", &[])),
    );

    // Kill it
    let resp = control_request(
        &broker.control_path(),
        &Request::Kill(KillRequest { session_id: sid }),
    );
    assert!(matches!(resp, Response::Ok(_)));

    // List should be empty
    let resp = control_request(&broker.control_path(), &Request::List);
    match resp {
        Response::List(list) => assert!(list.sessions.is_empty(), "session should be removed"),
        other => panic!("expected List, got: {:?}", other),
    }

    // HasSession should return false
    let resp = control_request(
        &broker.control_path(),
        &Request::HasSession(HasSessionRequest { session_id: sid }),
    );
    assert!(matches!(resp, Response::HasSession(r) if !r.alive));
}

#[test]
fn test_kill_nonexistent_session_returns_error() {
    let broker = start_broker();
    let resp = control_request(
        &broker.control_path(),
        &Request::Kill(KillRequest {
            session_id: Uuid::new_v4(),
        }),
    );
    assert!(
        matches!(resp, Response::Error(_)),
        "killing nonexistent session should error"
    );
}

#[test]
fn test_resize_session() {
    let broker = start_broker();
    let sid = Uuid::new_v4();

    control_request(
        &broker.control_path(),
        &Request::Spawn(make_spawn_request(sid, "/bin/cat", &[])),
    );

    let resp = control_request(
        &broker.control_path(),
        &Request::Resize(ResizeRequest {
            session_id: sid,
            rows: 48,
            cols: 120,
        }),
    );
    assert!(
        matches!(resp, Response::Ok(_)),
        "resize should succeed, got: {:?}",
        resp
    );
}

#[test]
fn test_resize_nonexistent_session_returns_error() {
    let broker = start_broker();
    let resp = control_request(
        &broker.control_path(),
        &Request::Resize(ResizeRequest {
            session_id: Uuid::new_v4(),
            rows: 24,
            cols: 80,
        }),
    );
    assert!(matches!(resp, Response::Error(_)));
}

#[test]
fn test_spawn_duplicate_session_is_idempotent() {
    let broker = start_broker();
    let sid = Uuid::new_v4();

    let resp1 = control_request(
        &broker.control_path(),
        &Request::Spawn(make_spawn_request(sid, "/bin/cat", &[])),
    );
    assert!(matches!(resp1, Response::Ok(_)));

    // Spawning the same session ID again should succeed (idempotent)
    let resp2 = control_request(
        &broker.control_path(),
        &Request::Spawn(make_spawn_request(sid, "/bin/cat", &[])),
    );
    assert!(matches!(resp2, Response::Ok(_)));

    // Should still be just one session
    let resp = control_request(&broker.control_path(), &Request::List);
    match resp {
        Response::List(list) => assert_eq!(list.sessions.len(), 1),
        other => panic!("expected List, got: {:?}", other),
    }
}

#[test]
fn test_ring_buffer_replay_on_reconnect() {
    let broker = start_broker();
    let sid = Uuid::new_v4();

    control_request(
        &broker.control_path(),
        &Request::Spawn(make_spawn_request(sid, "/bin/cat", &[])),
    );

    let data_path = broker.data_path(sid);

    // First connection: write some data and confirm it echoes
    {
        let mut data = connect_data_socket(&data_path);
        data.set_nonblocking(false).unwrap();
        data.set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();

        data.write_all(b"replay-test-data\n").unwrap();
        data.flush().unwrap();

        data.set_nonblocking(true).unwrap();
        let output = read_until(&mut data, "replay-test-data", Duration::from_secs(5));
        assert!(
            output.contains("replay-test-data"),
            "first connection should echo, got: {:?}",
            output
        );
        // Drop the connection
    }

    // Brief pause to let broker process the disconnect
    std::thread::sleep(Duration::from_millis(200));

    // Second connection: should get ring buffer replay
    let mut data2 = UnixStream::connect(&data_path).expect("reconnect data socket");
    data2.set_nonblocking(true).unwrap();

    let output = read_until(&mut data2, "replay-test-data", Duration::from_secs(5));
    assert!(
        output.contains("replay-test-data"),
        "reconnect should replay ring buffer, got: {:?}",
        output
    );
}

#[test]
fn test_shutdown_cleans_up() {
    let broker = start_broker();
    let control_path = broker.control_path();
    let pid_path = broker.socket_dir.join("pty-broker.pid");

    // Spawn a session so there's something to clean up
    let sid = Uuid::new_v4();
    control_request(
        &control_path,
        &Request::Spawn(make_spawn_request(sid, "/bin/cat", &[])),
    );

    let data_path = broker.data_path(sid);
    let deadline = Instant::now() + Duration::from_secs(3);
    while !data_path.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }

    // Send shutdown
    let resp = control_request(&control_path, &Request::Shutdown);
    assert!(matches!(resp, Response::Ok(_)));

    // Wait for broker process to exit
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        // Try connecting — should fail once broker is gone
        if UnixStream::connect(&control_path).is_err() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    // Control socket should be cleaned up
    assert!(
        !control_path.exists(),
        "control socket should be removed after shutdown"
    );
    // PID file should be cleaned up
    assert!(
        !pid_path.exists(),
        "PID file should be removed after shutdown"
    );
    // Data socket should be cleaned up
    assert!(
        !data_path.exists(),
        "data socket should be removed after shutdown"
    );
}

#[test]
fn test_multiple_sessions() {
    let broker = start_broker();
    let sid1 = Uuid::new_v4();
    let sid2 = Uuid::new_v4();

    control_request(
        &broker.control_path(),
        &Request::Spawn(make_spawn_request(sid1, "/bin/cat", &[])),
    );
    control_request(
        &broker.control_path(),
        &Request::Spawn(make_spawn_request(sid2, "/bin/cat", &[])),
    );

    let resp = control_request(&broker.control_path(), &Request::List);
    match resp {
        Response::List(list) => {
            assert_eq!(list.sessions.len(), 2);
            let ids: Vec<Uuid> = list.sessions.iter().map(|s| s.id).collect();
            assert!(ids.contains(&sid1));
            assert!(ids.contains(&sid2));
        }
        other => panic!("expected List, got: {:?}", other),
    }

    // Kill one, verify the other remains
    control_request(
        &broker.control_path(),
        &Request::Kill(KillRequest { session_id: sid1 }),
    );

    let resp = control_request(&broker.control_path(), &Request::List);
    match resp {
        Response::List(list) => {
            assert_eq!(list.sessions.len(), 1);
            assert_eq!(list.sessions[0].id, sid2);
        }
        other => panic!("expected List, got: {:?}", other),
    }
}

#[test]
fn test_session_detects_child_exit() {
    let broker = start_broker();
    let sid = Uuid::new_v4();

    // Spawn a short-lived command
    control_request(
        &broker.control_path(),
        &Request::Spawn(make_spawn_request(sid, "/bin/echo", &["done"])),
    );

    // Wait for the process to exit
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let resp = control_request(
            &broker.control_path(),
            &Request::HasSession(HasSessionRequest { session_id: sid }),
        );
        if matches!(resp, Response::HasSession(ref r) if !r.alive) {
            break;
        }
        if Instant::now() > deadline {
            panic!("session should have detected child exit");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn test_spawn_echo_reads_output_via_data_socket() {
    let broker = start_broker();
    let sid = Uuid::new_v4();

    // Use cat (long-lived) instead of echo so the session stays alive
    // long enough to connect the data socket.
    control_request(
        &broker.control_path(),
        &Request::Spawn(make_spawn_request(sid, "/bin/cat", &[])),
    );

    let mut data = connect_data_socket(&broker.data_path(sid));
    data.set_nonblocking(false).unwrap();
    data.set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();

    // Write through cat's stdin; cat echoes it back
    data.write_all(b"hello world\n")
        .expect("write to data socket");
    data.flush().unwrap();

    data.set_nonblocking(true).unwrap();
    let output = read_until(&mut data, "hello world", Duration::from_secs(5));
    assert!(
        output.contains("hello world"),
        "should read echo output, got: {:?}",
        output
    );
}
