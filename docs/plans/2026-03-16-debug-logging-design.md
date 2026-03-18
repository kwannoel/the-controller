# Debug Logging Design

## Problem

The Controller has no structured or persistent logging. All Rust debug output uses `eprintln!` (lost when the app closes), and the new PTY broker daemon double-forks to `/dev/null`, making its `eprintln!` calls completely invisible after daemonization. Frontend errors go through `console.error()` with a `logToBackend()` bridge that also writes to stderr.

## Decision

Use the `tracing` ecosystem (`tracing` + `tracing-subscriber` + `tracing-appender`) for structured, file-based logging across all three processes: Tauri main, PTY broker daemon, and server mode. Frontend errors get a separate log file via the existing backend bridge.

## Log Directory Structure

```
~/.the-controller/logs/
├── backend.log                              # current session — easy to find
├── broker.log
├── frontend.log
└── history/
    ├── backend-2026-03-16T10-30-00.0.log.gz     # all files have numeric suffix
    ├── backend-2026-03-16T10-30-00.1.log.gz     # second chunk (100MB rotation)
    ├── broker-2026-03-16T14-20-05.0.log.gz
    └── frontend-2026-03-16T10-30-00.0.log.gz
```

### Rules

- Current logs live at the top level (`backend.log`, `broker.log`, `frontend.log`)
- On app startup, any existing current log is archived to `history/` with gzip compression
- History files always have a numeric suffix: `<type>-<ISO8601>.<N>.log.gz`
- When a current log exceeds 100MB, it is rotated: archived to history, new file created
- On startup, files in `history/` older than 7 days are deleted

## Log Format

```
2026-03-16T10:30:05.123+08:00 INFO  [pty_manager] session spawned: id=abc123 shell=/bin/zsh
2026-03-16T10:30:05.456+08:00 DEBUG [broker_client] connecting to broker socket
2026-03-16T10:30:05.789+08:00 ERROR [voice::pipeline] whisper transcription failed: timeout
```

Format: `<ISO8601 timestamp> <LEVEL> [<module>] <message>`

## Log Level Control

Priority: `RUST_LOG` env var > `config.toml` > default `info`

- Environment variable: `RUST_LOG=info` (default), `RUST_LOG=debug`, `RUST_LOG=the_controller::broker_client=trace`
- Config file: `~/.the-controller/config.toml` → `log_level = "info"`

## Architecture

Three independent processes, each initializing tracing separately:

### 1. Tauri Main Process (`src-tauri/src/lib.rs`)

- Initialize tracing subscriber on startup: stderr layer + file layer (`backend.log`)
- Replace all `eprintln!` with `tracing::info!` / `tracing::error!` etc.
- `log_frontend_error` command writes to `frontend.log` (separate file appender)
- Startup: archive previous session logs + clean up history older than 7 days

### 2. PTY Broker Daemon (`src-tauri/src/bin/pty_broker.rs`)

- Critical: initialize file appender BEFORE `daemonize()` (stderr goes to `/dev/null` after)
- `--foreground` mode: stderr + file dual output
- Daemon mode: file output only (`broker.log`)

### 3. Server Mode (`src-tauri/src/bin/server.rs`)

- Shares initialization logic with Tauri main process
- Writes to `backend.log`

### Shared Module: `src-tauri/src/logging.rs` (new)

- `init_backend_logging(foreground: bool)` — initialize backend/server logging
- `init_broker_logging(foreground: bool)` — initialize broker logging
- `init_frontend_log_writer()` — return frontend.log writer
- `rotate_if_needed(path)` — check file size, archive if > 100MB
- `cleanup_old_logs(days: u64)` — delete history files older than N days
- `archive_current_log(path)` — move to history/ and gzip compress

## Cargo Dependencies

- `tracing` — macros and API
- `tracing-subscriber` — subscriber composition (`fmt` layer + `EnvFilter`)
- `flate2` — gzip compression for history logs

## Migration

- Global replace `eprintln!` → appropriate `tracing::warn!` / `tracing::error!` (by semantic meaning)
- `log_frontend_error` command internals changed to write `frontend.log`
- No external API or frontend behavior changes

## Testing

- Unit tests: `cleanup_old_logs` cleanup logic, `rotate_if_needed` size check, `archive_current_log` compression
- Integration test: start app → write logs → verify file exists with correct format
- Broker test: verify daemon mode writes to file, not stderr
