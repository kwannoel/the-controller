use axum::{
    body::Bytes,
    http::{Method, StatusCode},
};
use std::collections::HashMap;
use the_controller_lib::daemon_gateway::{
    daemon_socket_path, forward_http_for_test, forward_http_with_content_type, proxy_http_gateway,
    DaemonGatewayConfig,
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

#[test]
fn gateway_does_not_expose_daemon_token_route() {
    let main_source = include_str!("../src/main.rs");
    let commands_source = include_str!("../src/commands.rs");

    assert!(!main_source.contains("/api/read_daemon_token"));
    assert!(!main_source.contains("fn read_daemon_token"));
    assert!(!commands_source.contains("pub mod daemon"));
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

#[tokio::test]
async fn gateway_proxy_preserves_query_and_body_over_unix_socket() {
    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("daemon.sock");
    let server = serve_uds_request_echo(&socket).await;

    let response = proxy_http_gateway(
        &socket,
        Method::POST,
        "/api/daemon/sessions",
        Some("since=5&channels=stdout%2Cstderr"),
        Bytes::from_static(br#"{"cwd":"/tmp/project"}"#),
        Some("application/json".to_string()),
    )
    .await
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(response.content_type.as_deref(), Some("text/plain"));
    assert_eq!(
        response.body.as_ref(),
        br#"POST /sessions?since=5&channels=stdout%2Cstderr application/json {"cwd":"/tmp/project"}"#
    );
    server.abort();
}

#[tokio::test]
async fn gateway_proxy_maps_missing_socket_to_bad_gateway() {
    let tmp = tempfile::tempdir().unwrap();
    let missing_socket = tmp.path().join("missing-daemon.sock");

    let error = proxy_http_gateway(
        &missing_socket,
        Method::GET,
        "/api/daemon/health",
        None,
        Bytes::new(),
        None,
    )
    .await
    .unwrap_err();

    assert_eq!(error.0, StatusCode::BAD_GATEWAY);
    assert_eq!(error.1, "daemon gateway unavailable");
}

async fn serve_uds_health(socket: &std::path::Path) -> JoinHandle<()> {
    let listener = UnixListener::bind(socket).unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _addr)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let Some(request) = read_http_request(&mut stream).await else {
                    return;
                };
                let response = if request.method == "GET" && request.target == "/health" {
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
        let Some(request) = read_http_request(&mut stream).await else {
            return;
        };
        let content_type = request.header("content-type").unwrap_or_default();
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\n\r\n{}",
            content_type.len(),
            content_type
        );
        let _ = stream.write_all(response.as_bytes()).await;
    })
}

async fn serve_uds_request_echo(socket: &std::path::Path) -> JoinHandle<()> {
    let listener = UnixListener::bind(socket).unwrap();
    tokio::spawn(async move {
        let Ok((mut stream, _addr)) = listener.accept().await else {
            return;
        };
        let Some(request) = read_http_request(&mut stream).await else {
            return;
        };
        let content_type = request.header("content-type").unwrap_or_default();
        let body = String::from_utf8_lossy(&request.body);
        let response_body = format!(
            "{} {} {} {}",
            request.method, request.target, content_type, body
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        let _ = stream.write_all(response.as_bytes()).await;
    })
}

struct CapturedRequest {
    method: String,
    target: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl CapturedRequest {
    fn header(&self, name: &str) -> Option<String> {
        self.headers.get(&name.to_ascii_lowercase()).cloned()
    }
}

async fn read_http_request(stream: &mut tokio::net::UnixStream) -> Option<CapturedRequest> {
    let mut buffer = Vec::new();
    let header_end = loop {
        if let Some(index) = find_header_end(&buffer) {
            break index;
        }
        let mut chunk = [0; 512];
        let read = stream.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
    };

    let header_bytes = &buffer[..header_end];
    let header_text = String::from_utf8_lossy(header_bytes);
    let mut lines = header_text.split("\r\n");
    let request_line = lines.next()?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next()?.to_string();
    let target = request_parts.next()?.to_string();
    let mut headers = HashMap::new();

    for line in lines {
        if line.is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
    }

    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let body_start = header_end + 4;
    while buffer.len() < body_start + content_length {
        let mut chunk = [0; 512];
        let read = stream.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    let body = buffer[body_start..body_start + content_length].to_vec();

    Some(CapturedRequest {
        method,
        target,
        headers,
        body,
    })
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}
