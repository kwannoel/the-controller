use std::path::PathBuf;

pub(crate) fn daemon_token_path() -> PathBuf {
    if let Ok(dir) = std::env::var("TCD_STATE_DIR") {
        return PathBuf::from(dir).join("daemon.token");
    }
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join(".the-controller")
        .join("daemon.token")
}

pub(crate) fn read_token_from(path: &std::path::Path) -> Result<String, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("read daemon token at {}: {}", path.display(), e))?;
    let s = String::from_utf8(bytes).map_err(|e| format!("token not utf-8: {}", e))?;
    Ok(s.trim().to_string())
}

pub async fn read_daemon_token() -> Result<String, String> {
    let path = daemon_token_path();
    tokio::task::spawn_blocking(move || read_token_from(&path))
        .await
        .map_err(|e| format!("join error: {}", e))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn reads_and_trims_token() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("daemon.token");
        let mut f = std::fs::File::create(&p).unwrap();
        writeln!(f, "abc123").unwrap();
        assert_eq!(read_token_from(&p).unwrap(), "abc123");
    }

    #[test]
    fn missing_token_returns_err() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("daemon.token");
        let err = read_token_from(&p).unwrap_err();
        assert!(err.contains("daemon.token"), "got: {}", err);
    }
}
