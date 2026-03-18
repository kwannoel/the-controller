use std::net::TcpListener;
use std::process::Command;

use tempfile::TempDir;

#[cfg(feature = "server")]
#[test]
fn server_exits_cleanly_when_port_is_unavailable() {
    let occupied = TcpListener::bind("127.0.0.1:0").expect("bind occupied port");
    let port = occupied.local_addr().expect("occupied local addr").port();
    let home = TempDir::new().expect("temp home");

    let output = Command::new(env!("CARGO_BIN_EXE_server"))
        .env("HOME", home.path())
        .env("CONTROLLER_BIND", "127.0.0.1")
        .env("CONTROLLER_PORT", port.to_string())
        .output()
        .expect("run server binary");

    assert!(
        !output.status.success(),
        "server should exit non-zero when bind fails"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("panicked at"),
        "server should handle startup errors without panicking, stderr was: {stderr}"
    );
    assert!(
        stderr.contains("failed to start")
            || stderr.contains("Address already in use")
            || stderr.contains("address already in use"),
        "server should explain the startup failure, stderr was: {stderr}"
    );
}
