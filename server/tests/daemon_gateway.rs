use axum::{body::Bytes, http::Method};
use the_controller_lib::daemon_gateway::{
    daemon_socket_path, forward_http_for_test, forward_http_with_content_type, DaemonGatewayConfig,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixListener,
    task::JoinHandle,
};

#[test]
fn daemon_socket_path_uses_controller_state_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = DaemonGatewayConfig {
        state_dir: tmp.path().to_path_buf(),
    };

    assert_eq!(daemon_socket_path(&cfg), tmp.path().join("daemon.sock"));
}

#[test]
fn gateway_paths_must_stay_under_api_daemon() {
    assert_eq!(
        the_controller_lib::daemon_gateway::normalize_daemon_path("/api/daemon/profiles").unwrap(),
        "/profiles"
    );
    assert!(
        the_controller_lib::daemon_gateway::normalize_daemon_path("/api/list_projects").is_err()
    );
    assert!(the_controller_lib::daemon_gateway::normalize_daemon_path("/api/daemonish").is_err());
    assert!(
        the_controller_lib::daemon_gateway::normalize_daemon_path("/api/daemon/../projects")
            .is_err()
    );
    assert!(the_controller_lib::daemon_gateway::normalize_daemon_path(
        "/api/daemon/%2e%2e/projects"
    )
    .is_err());
    assert!(the_controller_lib::daemon_gateway::normalize_daemon_path(
        "/api/daemon/%2E%2E/projects"
    )
    .is_err());
}

#[tokio::test]
async fn gateway_health_uses_unix_socket() {
    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("daemon.sock");
    let server = serve_uds_health(&socket).await;

    let body = forward_http_for_test(&socket, "/health").await.unwrap();

    assert_eq!(body.status, 200);
    assert_eq!(body.body.as_ref(), b"ok");
    server.abort();
}

#[tokio::test]
async fn gateway_post_forwards_request_content_type() {
    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("daemon.sock");
    let server = serve_uds_content_type_echo(&socket).await;

    let body = forward_http_with_content_type(
        &socket,
        Method::POST,
        "/sessions".to_string(),
        Bytes::from_static(br#"{"cwd":"/tmp"}"#),
        Some("application/json".to_string()),
    )
    .await
    .unwrap();

    assert_eq!(body.status, 200);
    assert_eq!(body.body.as_ref(), b"application/json");
    server.abort();
}

async fn serve_uds_health(socket: &std::path::Path) -> JoinHandle<()> {
    let listener = UnixListener::bind(socket).unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _addr)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut request = [0; 1024];
                let Ok(read) = stream.read(&mut request).await else {
                    return;
                };
                let request = String::from_utf8_lossy(&request[..read]);
                let response = if request.starts_with("GET /health HTTP/") {
                    "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: 2\r\n\r\nok"
                } else {
                    "HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\n\r\n"
                };
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    })
}

async fn serve_uds_content_type_echo(socket: &std::path::Path) -> JoinHandle<()> {
    let listener = UnixListener::bind(socket).unwrap();
    tokio::spawn(async move {
        let Ok((mut stream, _addr)) = listener.accept().await else {
            return;
        };
        let mut request = [0; 2048];
        let Ok(read) = stream.read(&mut request).await else {
            return;
        };
        let request = String::from_utf8_lossy(&request[..read]);
        let content_type = request
            .lines()
            .find_map(|line| line.strip_prefix("content-type: "))
            .unwrap_or("");
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\n\r\n{}",
            content_type.len(),
            content_type
        );
        let _ = stream.write_all(response.as_bytes()).await;
    })
}
