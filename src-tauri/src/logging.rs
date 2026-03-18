use chrono::Local;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

pub const MAX_LOG_SIZE: u64 = 100 * 1024 * 1024; // 100 MB
pub const LOG_RETENTION_DAYS: u64 = 7;

// Rotation strategy:
//
// - On startup, the previous session's log is archived to history/ and gzip'd.
// - MAX_LOG_SIZE (100 MB) is a soft cap per session. If a single session
//   exceeds it, the file grows until the next restart. True mid-session
//   rotation would require reopening the tracing-appender file descriptor,
//   which adds significant complexity for little practical benefit (most
//   sessions won't generate 100 MB of logs).
// - History files older than LOG_RETENTION_DAYS (7 days) are cleaned up on
//   startup.

/// Ensure the logs directory and history subdirectory exist, return logs dir path.
pub fn logs_dir(base_dir: &Path) -> PathBuf {
    let dir = base_dir.join("logs");
    let history = dir.join("history");
    let _ = fs::create_dir_all(&history);
    dir
}

/// Archive a current log file to history/ with gzip compression.
pub fn archive_current_log(log_path: &Path, prefix: &str) -> io::Result<PathBuf> {
    if !log_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "log file not found",
        ));
    }
    let history = log_path.parent().unwrap().join("history");
    let _ = fs::create_dir_all(&history);

    let timestamp = Local::now().format("%Y-%m-%dT%H-%M-%S");
    let mut n = 0u32;
    loop {
        if n >= 1000 {
            return Err(io::Error::other(
                "too many archive files with same timestamp",
            ));
        }
        let name = format!("{}-{}.{}.log.gz", prefix, timestamp, n);
        let dest = history.join(&name);
        if !dest.exists() {
            let input = fs::File::open(log_path)?;
            let mut reader = BufReader::new(input);
            let output = fs::File::create(&dest)?;
            let mut encoder = GzEncoder::new(BufWriter::new(output), Compression::default());
            let mut buf = [0u8; 8192];
            loop {
                let bytes_read = reader.read(&mut buf)?;
                if bytes_read == 0 {
                    break;
                }
                encoder.write_all(&buf[..bytes_read])?;
            }
            encoder.finish()?;
            fs::remove_file(log_path)?;
            return Ok(dest);
        }
        n += 1;
    }
}

/// Delete history files older than `days` days.
pub fn cleanup_old_logs(history_dir: &Path, days: u64) {
    let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(days * 24 * 3600);
    if let Ok(entries) = fs::read_dir(history_dir) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if let Ok(modified) = meta.modified() {
                    if modified < cutoff {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }
    }
}

/// If the log file exceeds `max_bytes`, archive it and return true.
pub fn rotate_if_needed(log_path: &Path, prefix: &str, max_bytes: u64) -> bool {
    if let Ok(meta) = fs::metadata(log_path) {
        if meta.len() > max_bytes {
            let _ = archive_current_log(log_path, prefix);
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Tracing initialization
// ---------------------------------------------------------------------------

/// Holds the non-blocking writer guard. Dropping this flushes buffered log
/// output, so it must be kept alive for the lifetime of the process.
pub struct LogGuard {
    _guard: WorkerGuard,
}

/// Read the `log_level` field from `config.json` using raw `serde_json::Value`
/// to avoid coupling to the `Config` struct.
fn log_level_from_config(base_dir: &Path) -> Option<String> {
    let path = base_dir.join("config.json");
    let text = fs::read_to_string(path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&text).ok()?;
    val.get("log_level")?.as_str().map(|s| s.to_owned())
}

/// Build an `EnvFilter` with the following precedence:
///   1. `RUST_LOG` environment variable
///   2. `log_level` from config.json
///   3. `"info"` default
pub fn build_env_filter(base_dir: &Path) -> EnvFilter {
    if let Ok(rust_log) = std::env::var("RUST_LOG") {
        if let Ok(filter) = EnvFilter::try_new(&rust_log) {
            return filter;
        }
    }
    if let Some(level) = log_level_from_config(base_dir) {
        if let Ok(filter) = EnvFilter::try_new(&level) {
            return filter;
        }
    }
    EnvFilter::new("info")
}

/// Shared implementation for `init_backend_logging` and `init_broker_logging`.
///
/// - Archives the previous log file (if any).
/// - Cleans up old history entries.
/// - Creates a non-blocking file writer and builds a `tracing_subscriber` with
///   an `EnvFilter`, a file layer (no ANSI), and an optional stderr layer when
///   `foreground` is true.
fn init_logging(base_dir: &Path, log_name: &str, prefix: &str, foreground: bool) -> LogGuard {
    let logs = logs_dir(base_dir);
    let log_path = logs.join(log_name);

    // Archive previous log if it exists.
    if log_path.exists() {
        let _ = archive_current_log(&log_path, prefix);
    }
    cleanup_old_logs(&logs.join("history"), LOG_RETENTION_DAYS);

    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .expect("failed to open log file");

    let (non_blocking, guard) = tracing_appender::non_blocking(file);

    let filter = build_env_filter(base_dir);

    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true);

    let registry = tracing_subscriber::registry().with(filter).with(file_layer);

    if foreground {
        let stderr_layer = fmt::layer().with_writer(io::stderr).with_target(true);
        registry.with(stderr_layer).init();
    } else {
        registry.init();
    }

    LogGuard { _guard: guard }
}

/// Initialise tracing for the Tauri backend process.
///
/// Logs go to `<base_dir>/logs/backend.log`. When `foreground` is true an
/// additional stderr layer is added for interactive debugging.
pub fn init_backend_logging(base_dir: &Path, foreground: bool) -> LogGuard {
    init_logging(base_dir, "backend.log", "backend", foreground)
}

/// Initialise tracing for the PTY broker daemon.
///
/// Logs go to `<base_dir>/logs/broker.log`. When `foreground` is true an
/// additional stderr layer is added.
pub fn init_broker_logging(base_dir: &Path, foreground: bool) -> LogGuard {
    init_logging(base_dir, "broker.log", "broker", foreground)
}

/// Prepare a fresh `frontend.log` file for the frontend log bridge.
///
/// Archives the previous frontend.log (if any), then returns an open file
/// handle and its path. The caller writes frontend log lines into this file.
pub fn init_frontend_log_writer(base_dir: &Path) -> io::Result<(fs::File, PathBuf)> {
    let logs = logs_dir(base_dir);
    let log_path = logs.join("frontend.log");

    if log_path.exists() {
        let _ = archive_current_log(&log_path, "frontend");
    }
    cleanup_old_logs(&logs.join("history"), LOG_RETENTION_DAYS);

    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    Ok((file, log_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_logs_dir_created() {
        let tmp = TempDir::new().unwrap();
        let logs = logs_dir(tmp.path());
        assert!(logs.exists());
        assert!(logs.join("history").exists());
    }

    #[test]
    fn test_archive_current_log() {
        let tmp = TempDir::new().unwrap();
        let logs = logs_dir(tmp.path());
        let log_file = logs.join("backend.log");
        fs::write(&log_file, "line1\nline2\n").unwrap();

        archive_current_log(&log_file, "backend").unwrap();

        assert!(!log_file.exists());
        let history = logs.join("history");
        let entries: Vec<_> = fs::read_dir(&history).unwrap().collect();
        assert_eq!(entries.len(), 1);
        let name = entries[0].as_ref().unwrap().file_name();
        let name = name.to_str().unwrap();
        assert!(name.starts_with("backend-"));
        assert!(name.ends_with(".0.log.gz"));
    }

    #[test]
    fn test_cleanup_old_logs() {
        let tmp = TempDir::new().unwrap();
        let logs = logs_dir(tmp.path());
        let history = logs.join("history");

        let fresh = history.join("backend-2099-01-01T00-00-00.0.log.gz");
        fs::write(&fresh, "data").unwrap();

        let old = history.join("backend-2020-01-01T00-00-00.0.log.gz");
        fs::write(&old, "data").unwrap();
        let thirty_days_ago =
            std::time::SystemTime::now() - std::time::Duration::from_secs(30 * 24 * 3600);
        filetime::set_file_mtime(&old, filetime::FileTime::from_system_time(thirty_days_ago))
            .unwrap();

        cleanup_old_logs(&history, 7);

        assert!(fresh.exists());
        assert!(!old.exists());
    }

    #[test]
    fn test_rotate_if_needed_under_limit() {
        let tmp = TempDir::new().unwrap();
        let logs = logs_dir(tmp.path());
        let log_file = logs.join("backend.log");
        fs::write(&log_file, "small content").unwrap();

        let rotated = rotate_if_needed(&log_file, "backend", 100 * 1024 * 1024);
        assert!(!rotated);
        assert!(log_file.exists());
    }

    #[test]
    fn test_rotate_if_needed_over_limit() {
        let tmp = TempDir::new().unwrap();
        let logs = logs_dir(tmp.path());
        let log_file = logs.join("backend.log");
        fs::write(&log_file, "x".repeat(2000)).unwrap();

        let rotated = rotate_if_needed(&log_file, "backend", 1024);
        assert!(rotated);
        assert!(!log_file.exists());

        let history = logs.join("history");
        let entries: Vec<_> = fs::read_dir(&history).unwrap().collect();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_log_level_from_config_present() {
        let tmp = TempDir::new().unwrap();
        let config =
            r#"{"projects_root": "/tmp", "default_provider": "claude-code", "log_level": "debug"}"#;
        fs::write(tmp.path().join("config.json"), config).unwrap();
        assert_eq!(log_level_from_config(tmp.path()), Some("debug".to_owned()));
    }

    #[test]
    fn test_log_level_from_config_missing_field() {
        let tmp = TempDir::new().unwrap();
        let config = r#"{"projects_root": "/tmp"}"#;
        fs::write(tmp.path().join("config.json"), config).unwrap();
        assert_eq!(log_level_from_config(tmp.path()), None);
    }

    #[test]
    fn test_log_level_from_config_no_file() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(log_level_from_config(tmp.path()), None);
    }

    #[test]
    fn test_build_env_filter_defaults_to_info() {
        // Save and clear RUST_LOG to avoid interference
        let saved_rust_log = std::env::var("RUST_LOG").ok();
        std::env::remove_var("RUST_LOG");

        let tmp = TempDir::new().unwrap();
        // No config.json, no RUST_LOG — should get "info" default.
        let filter = build_env_filter(tmp.path());
        let display = format!("{}", filter);
        assert!(
            display.contains("info"),
            "expected 'info' in filter: {display}"
        );

        // Restore
        if let Some(val) = saved_rust_log {
            std::env::set_var("RUST_LOG", val);
        }
    }

    #[test]
    fn test_build_env_filter_reads_config() {
        // Save and clear RUST_LOG to avoid interference
        let saved_rust_log = std::env::var("RUST_LOG").ok();
        std::env::remove_var("RUST_LOG");

        let tmp = TempDir::new().unwrap();
        let config = r#"{"log_level": "trace"}"#;
        fs::write(tmp.path().join("config.json"), config).unwrap();
        let filter = build_env_filter(tmp.path());
        let display = format!("{}", filter);
        assert!(
            display.contains("trace"),
            "expected 'trace' in filter: {display}"
        );

        // Restore
        if let Some(val) = saved_rust_log {
            std::env::set_var("RUST_LOG", val);
        }
    }

    #[test]
    fn test_init_frontend_log_writer() {
        let tmp = TempDir::new().unwrap();
        let logs = logs_dir(tmp.path());

        // Create a pre-existing frontend.log to verify archival.
        let log_path = logs.join("frontend.log");
        fs::write(&log_path, "old data").unwrap();

        let (mut file, path) = init_frontend_log_writer(tmp.path()).unwrap();
        assert_eq!(path, log_path);

        // The old log should have been archived.
        let history: Vec<_> = fs::read_dir(logs.join("history"))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(history.len(), 1);

        // We should be able to write to the returned file.
        writeln!(file, "hello").unwrap();
        let content = fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("hello"));
    }
}
