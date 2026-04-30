use std::path::PathBuf;

use serde_json::Value;

#[derive(Debug, Clone)]
pub struct DaemonGatewayConfig {
    pub state_dir: PathBuf,
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

pub fn daemon_gateway_placeholder_response(daemon_path: &str) -> Result<Value, String> {
    if daemon_path != "/health" {
        return Err("daemon gateway proxy is not implemented yet".to_string());
    }

    Ok(serde_json::json!({
        "path": daemon_path,
        "status": "gateway-ready"
    }))
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
