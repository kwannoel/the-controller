# Debug Logging Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan task-by-task.

**Goal:** Add structured, persistent file-based logging to all three processes (Tauri app, PTY broker daemon, server mode) using the `tracing` ecosystem.

**Architecture:** New `logging.rs` module handles initialization, file rotation, and cleanup. Each process calls an init function early in startup. All `eprintln!` calls are replaced with `tracing` macros. Frontend errors bridge to a separate `frontend.log` via the existing `log_frontend_error` command.

**Tech Stack:** `tracing`, `tracing-subscriber` (fmt + EnvFilter), `flate2` (gzip), `chrono` (already in deps)

**Note:** The design doc mentions `config.toml` but the codebase uses `config.json` with serde_json. This plan uses `config.json` to match the existing convention.

---

### Task 1: Add Cargo Dependencies

**Files:**
- Modify: `src-tauri/Cargo.toml`

**Step 1: Add tracing, tracing-subscriber, and flate2 to Cargo.toml**

Add these under `[dependencies]`:

```toml
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt", "json"] }
flate2 = "1"
```

**Step 2: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles with no errors

**Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(logging): add tracing, tracing-subscriber, and flate2 dependencies"
```

---

### Task 2: Create `logging.rs` — Log Directory and Cleanup Utilities

**Files:**
- Create: `src-tauri/src/logging.rs`
- Test: `src-tauri/src/logging.rs` (unit tests in same file)

**Step 1: Write failing tests for log directory and cleanup functions**

Create `src-tauri/src/logging.rs` with test module only:

```rust
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

        // Original file should be gone
        assert!(!log_file.exists());
        // History dir should have one .gz file with numeric suffix
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

        // Create a "fresh" file
        let fresh = history.join("backend-2099-01-01T00-00-00.0.log.gz");
        fs::write(&fresh, "data").unwrap();

        // Create an "old" file and backdate its mtime
        let old = history.join("backend-2020-01-01T00-00-00.0.log.gz");
        fs::write(&old, "data").unwrap();
        // Set mtime to 30 days ago
        let thirty_days_ago = std::time::SystemTime::now()
            - std::time::Duration::from_secs(30 * 24 * 3600);
        filetime::set_file_mtime(
            &old,
            filetime::FileTime::from_system_time(thirty_days_ago),
        )
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
        // Write > 1KB (use small limit for test)
        fs::write(&log_file, "x".repeat(2000)).unwrap();

        let rotated = rotate_if_needed(&log_file, "backend", 1024);
        assert!(rotated);
        assert!(!log_file.exists());

        let history = logs.join("history");
        let entries: Vec<_> = fs::read_dir(&history).unwrap().collect();
        assert_eq!(entries.len(), 1);
    }
}
```

**Step 2: Add tempfile and filetime as dev-dependencies in Cargo.toml**

```toml
[dev-dependencies]
tempfile = "3"
filetime = "0.2"
```

**Step 3: Run tests to verify they fail**

Run: `cd src-tauri && cargo test --lib logging::tests`
Expected: FAIL — functions don't exist yet

**Step 4: Implement the utility functions**

```rust
use chrono::Local;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

const MAX_LOG_SIZE: u64 = 100 * 1024 * 1024; // 100 MB
const LOG_RETENTION_DAYS: u64 = 7;

/// Ensure the logs directory and history subdirectory exist, return logs dir path.
pub fn logs_dir(base_dir: &Path) -> PathBuf {
    let dir = base_dir.join("logs");
    let history = dir.join("history");
    let _ = fs::create_dir_all(&history);
    dir
}

/// Archive a current log file to history/ with gzip compression.
/// Returns the path of the archived file.
pub fn archive_current_log(log_path: &Path, prefix: &str) -> io::Result<PathBuf> {
    if !log_path.exists() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "log file not found"));
    }
    let history = log_path.parent().unwrap().join("history");
    let _ = fs::create_dir_all(&history);

    let timestamp = Local::now().format("%Y-%m-%dT%H-%M-%S");
    // Find next available numeric suffix
    let mut n = 0u32;
    loop {
        let name = format!("{}-{}.{}.log.gz", prefix, timestamp, n);
        let dest = history.join(&name);
        if !dest.exists() {
            // Compress and write
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
    let cutoff = std::time::SystemTime::now()
        - std::time::Duration::from_secs(days * 24 * 3600);
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
```

**Step 5: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --lib logging::tests`
Expected: all 4 tests PASS

**Step 6: Commit**

```bash
git add src-tauri/src/logging.rs src-tauri/Cargo.toml
git commit -m "feat(logging): add log directory, archive, rotation, and cleanup utilities"
```

---

### Task 3: Create `logging.rs` — Tracing Initialization Functions

**Files:**
- Modify: `src-tauri/src/logging.rs`

**Step 1: Write a test for backend logging initialization**

Add to the test module in `logging.rs`:

```rust
    #[test]
    fn test_init_backend_logging_creates_log_file() {
        let tmp = TempDir::new().unwrap();
        let logs = logs_dir(tmp.path());

        // Initialize logging — should not panic
        let _guard = init_backend_logging(tmp.path(), true);

        // Write a log line
        tracing::info!("test message");

        // Flush by dropping guard
        drop(_guard);

        let log_file = logs.join("backend.log");
        assert!(log_file.exists());
        let content = fs::read_to_string(&log_file).unwrap();
        assert!(content.contains("test message"));
        assert!(content.contains("INFO"));
    }
```

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test --lib logging::tests::test_init_backend_logging`
Expected: FAIL — `init_backend_logging` doesn't exist

**Step 3: Implement tracing initialization**

Add to `logging.rs`:

```rust
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use std::sync::Mutex;

/// Guard that must be held for the lifetime of the logging system.
/// Dropping it flushes and closes log files.
pub struct LogGuard {
    _file_guard: tracing_appender::non_blocking::WorkerGuard,
}

/// Read log_level from config.json in base_dir, fallback to "info".
fn log_level_from_config(base_dir: &Path) -> String {
    let config_path = base_dir.join("config.json");
    if let Ok(content) = fs::read_to_string(&config_path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(level) = val.get("log_level").and_then(|v| v.as_str()) {
                return level.to_string();
            }
        }
    }
    "info".to_string()
}

/// Build an EnvFilter: RUST_LOG env var takes priority, then config, then "info".
fn build_env_filter(base_dir: &Path) -> EnvFilter {
    if std::env::var("RUST_LOG").is_ok() {
        EnvFilter::from_default_env()
    } else {
        let level = log_level_from_config(base_dir);
        EnvFilter::new(level)
    }
}

/// Initialize backend/server logging. Returns a guard that must be held.
/// If `foreground` is true, also logs to stderr.
pub fn init_backend_logging(base_dir: &Path, foreground: bool) -> LogGuard {
    let logs = logs_dir(base_dir);

    // Archive previous session's log if it exists
    let log_path = logs.join("backend.log");
    if log_path.exists() {
        let _ = archive_current_log(&log_path, "backend");
    }

    // Clean up old history
    cleanup_old_logs(&logs.join("history"), LOG_RETENTION_DAYS);

    // Create file appender
    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .expect("failed to open backend.log");
    let (non_blocking, file_guard) = tracing_appender::non_blocking(file);

    let filter = build_env_filter(base_dir);

    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true);

    if foreground {
        let stderr_layer = fmt::layer()
            .with_writer(std::io::stderr)
            .with_target(true);
        tracing_subscriber::registry()
            .with(filter)
            .with(file_layer)
            .with(stderr_layer)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(file_layer)
            .init();
    }

    LogGuard { _file_guard: file_guard }
}

/// Initialize broker daemon logging. Returns a guard that must be held.
/// MUST be called BEFORE daemonize() since stderr is redirected to /dev/null.
pub fn init_broker_logging(base_dir: &Path, foreground: bool) -> LogGuard {
    let logs = logs_dir(base_dir);

    let log_path = logs.join("broker.log");
    if log_path.exists() {
        let _ = archive_current_log(&log_path, "broker");
    }

    cleanup_old_logs(&logs.join("history"), LOG_RETENTION_DAYS);

    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .expect("failed to open broker.log");
    let (non_blocking, file_guard) = tracing_appender::non_blocking(file);

    let filter = build_env_filter(base_dir);

    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true);

    if foreground {
        let stderr_layer = fmt::layer()
            .with_writer(std::io::stderr)
            .with_target(true);
        tracing_subscriber::registry()
            .with(filter)
            .with(file_layer)
            .with(stderr_layer)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(file_layer)
            .init();
    }

    LogGuard { _file_guard: file_guard }
}

/// Create a writer for frontend.log. Returns a guard and the writer.
/// The caller writes frontend error messages to this writer.
pub fn init_frontend_log_writer(base_dir: &Path) -> io::Result<(fs::File, PathBuf)> {
    let logs = logs_dir(base_dir);
    let log_path = logs.join("frontend.log");

    // Archive previous session's frontend log
    if log_path.exists() {
        let _ = archive_current_log(&log_path, "frontend");
    }

    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    Ok((file, log_path))
}
```

**Step 4: Add `tracing-appender` dependency to Cargo.toml**

```toml
tracing-appender = "0.2"
```

**Step 5: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --lib logging::tests`
Expected: all tests PASS

**Step 6: Commit**

```bash
git add src-tauri/src/logging.rs src-tauri/Cargo.toml
git commit -m "feat(logging): add tracing initialization for backend, broker, and frontend"
```

---

### Task 4: Register `logging` Module and Wire Up Tauri Main Process

**Files:**
- Modify: `src-tauri/src/lib.rs` (add `mod logging;` and call init)
- Modify: `src-tauri/src/state.rs` (expose base_dir for logging init)

**Step 1: Add `mod logging;` to lib.rs**

In `src-tauri/src/lib.rs`, add near the other `mod` declarations:

```rust
mod logging;
```

**Step 2: Initialize logging early in `run()`**

In `src-tauri/src/lib.rs`, after `shell_env::inherit_shell_env()` (line 37) and before `tauri::Builder::default()` (line 39), add:

```rust
    // Initialize logging — must happen before any tracing macros are used
    let base_dir = storage::Storage::with_default_path()
        .map(|s| s.base_dir())
        .unwrap_or_else(|_| PathBuf::from("."));
    let _log_guard = logging::init_backend_logging(&base_dir, true);
```

The `_log_guard` must live for the duration of `run()` — it's held by the function scope.

**Step 3: Store frontend log writer in AppState**

In `src-tauri/src/state.rs`, add a field to `AppState`:

```rust
pub frontend_log: Mutex<Option<std::fs::File>>,
```

Initialize it in `AppState::new()` or `from_storage()`:

```rust
let frontend_log = match logging::init_frontend_log_writer(&storage.base_dir()) {
    Ok((file, _path)) => Mutex::new(Some(file)),
    Err(_) => Mutex::new(None),
};
```

**Step 4: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles with no errors

**Step 5: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/state.rs
git commit -m "feat(logging): wire up tracing init in Tauri main process"
```

---

### Task 5: Wire Up PTY Broker Daemon Logging

**Files:**
- Modify: `src-tauri/src/bin/pty_broker.rs`

**Step 1: Initialize logging before daemonize()**

In `pty_broker.rs` `main()`, after parsing args but BEFORE calling `daemonize()`, add:

```rust
    let base_dir = dirs::home_dir()
        .map(|h| h.join(".the-controller"))
        .unwrap_or_else(|| PathBuf::from("."));
    let foreground = args.contains(&"--foreground".to_string());
    let _log_guard = the_controller_lib::logging::init_broker_logging(&base_dir, foreground);
```

**Step 2: Replace all `eprintln!` in pty_broker.rs with tracing macros**

Replace these 5 occurrences:

| Line | Old | New |
|------|-----|-----|
| 443 | `println!("{}", env!("BUILD_DATE"));` | Keep as-is (stdout for CLI output, not logging) |
| 461 | `eprintln!("failed to create socket dir: {}", e);` | `tracing::error!("failed to create socket dir: {}", e);` |
| 469 | `eprintln!("failed to daemonize: {}", e);` | `tracing::error!("failed to daemonize: {}", e);` |
| 484 | `eprintln!("failed to write pid file: {}", e);` | `tracing::error!("failed to write pid file: {}", e);` |
| 495 | `eprintln!("failed to bind control socket: {}", e);` | `tracing::error!("failed to bind control socket: {}", e);` |

**Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check --bin pty_broker`
Expected: compiles with no errors

**Step 4: Commit**

```bash
git add src-tauri/src/bin/pty_broker.rs
git commit -m "feat(logging): wire up tracing in PTY broker daemon"
```

---

### Task 6: Wire Up Server Mode Logging

**Files:**
- Modify: `src-tauri/src/bin/server.rs`

**Step 1: Initialize logging at server startup**

In `server.rs` `main()`, early in the function (after arg parsing, before server setup), add:

```rust
    let base_dir = dirs::home_dir()
        .map(|h| h.join(".the-controller"))
        .unwrap_or_else(|| PathBuf::from("."));
    let _log_guard = the_controller_lib::logging::init_backend_logging(&base_dir, true);
```

**Step 2: Replace eprintln!/println! in server.rs with tracing macros**

| Line | Old | New |
|------|-----|-----|
| 62 | `println!("\nShutting down...");` | `tracing::info!("shutting down");` |
| 218 | `println!("Server listening on http://{}?token={}", addr, t);` | `tracing::info!("server listening on http://{}?token={}", addr, t);` |
| 219 | `println!("Server listening on http://{} (no auth)", addr);` | `tracing::info!("server listening on http://{} (no auth)", addr);` |
| 306-309 | `eprintln!("Failed to migrate worktrees...");` | `tracing::error!("failed to migrate worktrees for project '{}': {}", project.name, e);` |
| 1017 | `eprintln!("[FRONTEND] {}", message);` | Write to frontend log file (see Task 8) |
| 1255 | `eprintln!("notes git commit failed: {}", e);` | `tracing::error!("notes git commit failed: {}", e);` |

**Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check --features server`
Expected: compiles with no errors

**Step 4: Commit**

```bash
git add src-tauri/src/bin/server.rs
git commit -m "feat(logging): wire up tracing in server mode"
```

---

### Task 7: Migrate `eprintln!` in Core Modules

**Files:**
- Modify: `src-tauri/src/lib.rs` (2 occurrences)
- Modify: `src-tauri/src/commands.rs` (2 occurrences)
- Modify: `src-tauri/src/keybindings.rs` (4 occurrences)
- Modify: `src-tauri/src/cli_install.rs` (3 occurrences)
- Modify: `src-tauri/src/status_socket.rs` (18 occurrences)
- Modify: `src-tauri/src/storage.rs` (2 occurrences)
- Modify: `src-tauri/src/pty_manager.rs` (1 occurrence)
- Modify: `src-tauri/src/maintainer.rs` (1 occurrence)

**Step 1: Add `use tracing;` to each file and replace eprintln! calls**

Each file needs `use tracing::{error, warn, info};` (or just use fully qualified `tracing::error!`).

Mapping by file (see design doc exploration for exact lines):

**lib.rs:**
- `eprintln!("Failed to initialize app storage: {error}")` → `tracing::error!("failed to initialize app storage: {error}");`
- `eprintln!("Failed to lock storage for keybindings setup: {e}")` → `tracing::error!("failed to lock storage for keybindings setup: {e}");`

**commands.rs:**
- `eprintln!("Failed to migrate worktrees...")` → `tracing::error!(...)`
- `eprintln!("[FRONTEND] {}", message)` → Write to frontend log file (Task 8)

**keybindings.rs:** (all → `tracing::error!`)
- `eprintln!("Failed to write keybindings template...")`
- `eprintln!("Failed to create keybindings file watcher...")`
- `eprintln!("Failed to watch keybindings directory...")`
- `eprintln!("Keybindings watcher error...")`

**cli_install.rs:** (all → `tracing::warn!`)
- `eprintln!("Warning: could not determine home directory...")`
- `eprintln!("Warning: could not create {}...")`
- `eprintln!("Warning: could not install {}...")`

**status_socket.rs:** (18 occurrences — mostly `tracing::error!`, 2 are `tracing::warn!`)
- Lines with "Warning: another instance" → `tracing::warn!`
- All others → `tracing::error!`

**storage.rs:**
- `eprintln!("Warning: {}: failed to parse...")` → `tracing::warn!`
- `eprintln!("Warning: cannot migrate worktrees...")` → `tracing::warn!`

**pty_manager.rs:**
- `eprintln!("broker session failed, falling back...")` → `tracing::warn!`

**maintainer.rs:**
- `eprintln!("Maintainer check failed...")` → `tracing::error!`

**Step 2: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles with no errors

**Step 3: Commit**

```bash
git add src-tauri/src/
git commit -m "refactor(logging): replace eprintln! with tracing macros in core modules"
```

---

### Task 8: Migrate `eprintln!` in Auto-Worker and Voice Modules

**Files:**
- Modify: `src-tauri/src/auto_worker.rs` (13 occurrences)
- Modify: `src-tauri/src/voice/mod.rs` (5 occurrences)
- Modify: `src-tauri/src/voice/audio_input.rs` (2 occurrences)
- Modify: `src-tauri/src/voice/audio_output.rs` (2 occurrences)
- Modify: `src-tauri/src/voice/llm.rs` (1 occurrence)

**Step 1: Replace auto_worker.rs eprintln! calls**

All auto-worker calls use `Auto-worker:` prefix. Map them:
- "removing stale in-progress label" → `tracing::info!`
- "session timed out" → `tracing::info!`
- "killed after N nudges" → `tracing::info!`
- "session completed" → `tracing::info!`
- "failed to fetch issues" → `tracing::error!`
- "failed to spawn session" → `tracing::error!`
- "failed to restore session" → `tracing::error!`
- "nudged session" → `tracing::info!`
- "gh pr list failed" → `tracing::error!`
- "failed to run gh pr list" → `tracing::error!`
- "finalized as completed" → `tracing::info!`
- "exited while still open" → `tracing::info!`
- "cleanup could not confirm issue state" → `tracing::warn!`

**Step 2: Replace voice module eprintln! calls**

**voice/mod.rs:**
- `[voice] Pipeline error` → `tracing::error!(target: "voice", ...)`
- `[voice] You: {text}` → `tracing::info!(target: "voice", ...)`
- `[voice] Assistant: {full_response}` → `tracing::info!(target: "voice", ...)`
- `[voice] TTS error` → `tracing::error!(target: "voice", ...)`
- `[voice] Assistant (interrupted)` → `tracing::info!(target: "voice", ...)`

**voice/audio_input.rs:**
- `[voice] Mic: ...` → `tracing::info!(target: "voice", ...)`
- `[voice] Audio input error` → `tracing::error!(target: "voice", ...)`

**voice/audio_output.rs:**
- `[voice] Audio output error` → `tracing::error!(target: "voice", ...)`
- `[voice] Streaming audio error` → `tracing::error!(target: "voice", ...)`

**voice/llm.rs:**
- `[voice] Codex error (will retry)` → `tracing::warn!(target: "voice", ...)`

**Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles with no errors

**Step 4: Commit**

```bash
git add src-tauri/src/auto_worker.rs src-tauri/src/voice/
git commit -m "refactor(logging): replace eprintln! with tracing macros in auto-worker and voice"
```

---

### Task 9: Frontend Log Bridge — Write to `frontend.log`

**Files:**
- Modify: `src-tauri/src/commands.rs` — `log_frontend_error` command
- Modify: `src-tauri/src/bin/server.rs` — server-mode equivalent
- Modify: `src-tauri/src/state.rs` — add frontend log writer to AppState

**Step 1: Modify `log_frontend_error` in commands.rs**

Replace the `eprintln!("[FRONTEND] {}", message)` with writing to the frontend log file:

```rust
#[tauri::command]
pub fn log_frontend_error(message: String, state: tauri::State<'_, AppState>) {
    use std::io::Write;
    let timestamp = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%:z");
    let line = format!("{} ERROR [frontend] {}\n", timestamp, message);

    if let Ok(mut guard) = state.frontend_log.lock() {
        if let Some(ref mut file) = *guard {
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
    }

    // Also log via tracing so it appears in backend.log
    tracing::error!(target: "frontend", "{}", message);
}
```

**Step 2: Do the same for server.rs `log_frontend_error`**

Same pattern but using the server's AppState.

**Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles with no errors

**Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/bin/server.rs src-tauri/src/state.rs
git commit -m "feat(logging): write frontend errors to frontend.log"
```

---

### Task 10: Add `log_level` to Config

**Files:**
- Modify: `src-tauri/src/config.rs`

**Step 1: Add `log_level` field to Config struct**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub projects_root: String,
    #[serde(default)]
    pub default_provider: ConfigDefaultProvider,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_log_level() -> String {
    "info".to_string()
}
```

**Step 2: Verify it compiles and existing config.json still loads**

Run: `cd src-tauri && cargo check`
Expected: compiles — `#[serde(default)]` ensures backward compatibility with existing config files that don't have `log_level`.

**Step 3: Commit**

```bash
git add src-tauri/src/config.rs
git commit -m "feat(logging): add log_level field to config"
```

---

### Task 11: Add Log Rotation Check on Write (Size-Based)

**Files:**
- Modify: `src-tauri/src/logging.rs`

**Step 1: Add a periodic rotation check**

The tracing file appender writes continuously. We need to check file size periodically and rotate when it exceeds 100MB. Add a background task that checks every 60 seconds:

```rust
use std::sync::Arc;
use tokio::time::{interval, Duration};

/// Spawn a background task that checks log file size and rotates if needed.
/// Returns a JoinHandle that can be aborted on shutdown.
pub fn spawn_rotation_checker(
    log_path: PathBuf,
    prefix: String,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(60));
        loop {
            ticker.tick().await;
            if rotate_if_needed(&log_path, &prefix, MAX_LOG_SIZE) {
                // After rotation, the old file handle is still open but pointing
                // at the renamed file. We need to reopen. Since tracing-appender
                // uses non_blocking writer, we'd need to signal it.
                // For simplicity: log a warning. The file will be recreated
                // on next app restart. For mid-session rotation, the appender
                // continues writing to the (now archived) file descriptor.
                tracing::warn!("log file rotated — new entries continue in current fd until restart");
            }
        }
    })
}
```

**Note:** True mid-session rotation with tracing-appender requires reopening the file descriptor. A simpler approach: just check on startup and let the 100MB limit be a soft cap per session. If the user generates >100MB in a single session, it stays in one file until next restart. This avoids complexity. The rotation on startup handles the common case.

**Step 2: Decide on approach**

Given the complexity of mid-session fd rotation with tracing's non-blocking writer, the pragmatic approach is:
- Rotate on startup only (archive previous session's log)
- 100MB is a soft cap — if a single session exceeds it, the file grows until next restart
- This is acceptable because most sessions won't generate 100MB of logs

If mid-session rotation is needed later, it can be added by using `tracing-appender::rolling` with a custom trigger.

**Step 3: Commit (if any changes)**

```bash
git add src-tauri/src/logging.rs
git commit -m "docs(logging): document rotation strategy — startup-only with soft 100MB cap"
```

---

### Task 12: Run All Checks and Final Verification

**Files:** None (verification only)

**Step 1: Run Rust tests**

Run: `cd src-tauri && cargo test`
Expected: all tests pass

**Step 2: Run frontend tests**

Run: `pnpm test`
Expected: all tests pass

**Step 3: Run format and lint checks**

Run: `pnpm check`
Run: `cd src-tauri && cargo fmt --check`
Run: `cd src-tauri && cargo clippy -- -D warnings`
Expected: all pass

**Step 4: Manual smoke test**

Run: `pnpm tauri dev`
- Verify `~/.the-controller/logs/backend.log` is created
- Verify log lines have correct format: `<timestamp> <LEVEL> [<module>] <message>`
- Verify `~/.the-controller/logs/frontend.log` is created when a frontend error occurs
- Kill and restart — verify previous `backend.log` is archived to `history/` as `.0.log.gz`

**Step 5: Final commit if any fixups needed**

```bash
git add -A
git commit -m "fix(logging): address lint/test issues from final verification"
```
