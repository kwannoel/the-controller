use std::path::PathBuf;

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
    if rest.contains("..") {
        return Err("daemon gateway path must not contain ..".to_string());
    }
    Ok(if rest.is_empty() {
        "/".to_string()
    } else {
        rest.to_string()
    })
}
