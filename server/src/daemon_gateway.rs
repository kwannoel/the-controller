use std::path::{Path, PathBuf};

use axum::{
    body::Bytes,
    http::{Method, StatusCode},
};
use tokio::net::UnixStream;
use tokio_tungstenite::{client_async, WebSocketStream};

#[derive(Debug, Clone)]
pub struct DaemonGatewayConfig {
    pub state_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonResponse {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Bytes,
}

pub fn daemon_socket_path(cfg: &DaemonGatewayConfig) -> PathBuf {
    cfg.state_dir.join("daemon.sock")
}

pub fn normalize_daemon_path(path: &str) -> Result<String, String> {
    let rest = path
        .strip_prefix("/api/daemon")
        .ok_or_else(|| "path must start with /api/daemon".to_string())?;
    if !rest.is_empty() && !rest.starts_with('/') {
        return Err("path must stay under /api/daemon".to_string());
    }
    let decoded_rest = percent_decode(rest)?;
    if decoded_rest.contains("..") {
        return Err("daemon gateway path must not contain ..".to_string());
    }
    Ok(if rest.is_empty() {
        "/".to_string()
    } else {
        rest.to_string()
    })
}

pub fn is_daemon_stream_path(path: &str) -> bool {
    let Ok(daemon_path) = normalize_daemon_path(path) else {
        return false;
    };
    let segments: Vec<_> = daemon_path.trim_start_matches('/').split('/').collect();

    matches!(
        segments.as_slice(),
        ["chats", id, "stream"] | ["sessions", id, "stream"] if !id.is_empty()
    )
}

pub fn is_allowed_daemon_stream_origin(origin: Option<&str>, host: Option<&str>) -> bool {
    let Some(origin) = origin else {
        return true;
    };
    let Some(request_authority) = host.and_then(parse_authority) else {
        return false;
    };
    let Some(origin) = parse_origin(origin) else {
        return false;
    };

    origin.authority == request_authority && is_trusted_daemon_stream_host(&request_authority.host)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OriginParts {
    authority: AuthorityParts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AuthorityParts {
    host: String,
    port: Option<u16>,
}

fn parse_origin(origin: &str) -> Option<OriginParts> {
    let (scheme, authority) = origin.split_once("://")?;
    if !matches!(scheme, "http" | "https") {
        return None;
    }
    if authority.is_empty()
        || authority.contains('/')
        || authority.contains('?')
        || authority.contains('#')
    {
        return None;
    }

    Some(OriginParts {
        authority: parse_authority(authority)?,
    })
}

fn parse_authority(authority: &str) -> Option<AuthorityParts> {
    let authority: axum::http::uri::Authority = authority.parse().ok()?;

    Some(AuthorityParts {
        host: authority.host().to_ascii_lowercase(),
        port: authority.port_u16(),
    })
}

fn is_trusted_daemon_stream_host(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1" | "[::1]")
}

pub async fn connect_daemon_websocket(
    socket_path: &Path,
    daemon_path: &str,
) -> Result<WebSocketStream<UnixStream>, String> {
    let stream = UnixStream::connect(socket_path)
        .await
        .map_err(|e| format!("connect to daemon websocket: {e}"))?;
    let url = format!("ws://daemon.local{daemon_path}");
    let (ws, _response) = client_async(url, stream)
        .await
        .map_err(|e| format!("open daemon websocket: {e}"))?;

    Ok(ws)
}

pub async fn forward_http(
    socket_path: &Path,
    method: Method,
    daemon_path: String,
    body: Bytes,
) -> Result<DaemonResponse, String> {
    forward_http_with_content_type(socket_path, method, daemon_path, body, None).await
}

pub async fn forward_http_with_content_type(
    socket_path: &Path,
    method: Method,
    daemon_path: String,
    body: Bytes,
    content_type: Option<String>,
) -> Result<DaemonResponse, String> {
    let method = reqwest::Method::from_bytes(method.as_str().as_bytes())
        .map_err(|e| format!("invalid daemon gateway method: {e}"))?;
    let url = format!("http://daemon.local{daemon_path}");
    let client = reqwest::Client::builder()
        .unix_socket(socket_path)
        .build()
        .map_err(|e| format!("build daemon gateway client: {e}"))?;
    let mut request = client.request(method, url).body(body);
    if let Some(content_type) = content_type {
        request = request.header(reqwest::header::CONTENT_TYPE, content_type);
    }
    let response = request
        .send()
        .await
        .map_err(|e| format!("connect to daemon gateway: {e}"))?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let body = response
        .bytes()
        .await
        .map_err(|e| format!("read daemon gateway response: {e}"))?;

    Ok(DaemonResponse {
        status,
        content_type,
        body,
    })
}

pub async fn proxy_http_gateway(
    socket_path: &Path,
    method: Method,
    original_path: &str,
    query: Option<&str>,
    body: Bytes,
    content_type: Option<String>,
) -> Result<DaemonResponse, (StatusCode, String)> {
    let mut daemon_path =
        normalize_daemon_path(original_path).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    if let Some(query) = query {
        daemon_path.push('?');
        daemon_path.push_str(query);
    }

    forward_http_with_content_type(socket_path, method, daemon_path, body, content_type)
        .await
        .map_err(|_| {
            (
                StatusCode::BAD_GATEWAY,
                "daemon gateway unavailable".to_string(),
            )
        })
}

pub async fn forward_http_for_test(
    socket_path: &Path,
    daemon_path: &str,
) -> Result<DaemonResponse, String> {
    forward_http(
        socket_path,
        Method::GET,
        daemon_path.to_string(),
        Bytes::new(),
    )
    .await
}

fn percent_decode(input: &str) -> Result<String, String> {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err("daemon gateway path contains invalid percent escape".to_string());
            }
            let high = hex_value(bytes[index + 1])
                .ok_or_else(|| "daemon gateway path contains invalid percent escape".to_string())?;
            let low = hex_value(bytes[index + 2])
                .ok_or_else(|| "daemon gateway path contains invalid percent escape".to_string())?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(decoded)
        .map_err(|_| "daemon gateway path contains invalid utf-8 escape".to_string())
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
