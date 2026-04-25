# Domain Knowledge

Lessons learned during development. Check this before making changes.

## Axum handlers: Offload blocking work

**Problem:** Synchronous code inside an axum handler blocks the tokio reactor thread. Subprocess calls, file I/O, and git operations can starve other in-flight requests and the WebSocket broadcaster.

**Fix:** Use `tokio::task::spawn_blocking` for CPU- or IO-bound work:

```rust
// BAD: blocks the tokio thread
async fn slow_handler() -> Result<Json<Value>, (StatusCode, String)> {
    let result = expensive_operation(); // starves the reactor
    Ok(Json(result))
}

// GOOD: runs on the blocking thread pool
async fn slow_handler() -> Result<Json<Value>, (StatusCode, String)> {
    let result = tokio::task::spawn_blocking(expensive_operation)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("task failed: {e}")))?;
    Ok(Json(result))
}
```

**Rule of thumb:** Any handler that shells out (`Command::new(...)`) or does significant I/O must be async + spawn_blocking.

**Historical note:** This rule originated in the Tauri era (synchronous `#[tauri::command]` functions ran on the webview's main thread and froze the UI). The same hazard exists under axum — the blocking call just starves tokio instead.

## tmux Session Architecture

Sessions use tmux for process persistence. Two-layer PTY:

1. **tmux session** (`ctrl-{uuid}`): runs `claude` in a detached tmux session. Survives app exit.
2. **Attachment PTY**: local `portable-pty` running `tmux attach -t ctrl-{uuid}`. Reader thread reads from this. Dropped on app exit, re-created on restart.

Key behaviors:
- `spawn_session`: creates tmux session (if not exists) + attaches via local PTY
- `close_session`: kills tmux session + drops attachment PTY
- Intentional quit (`RunEvent::ExitRequested`): kills all tmux sessions
- Dev restart (process killed): no cleanup runs → tmux sessions survive → app reattaches on restart
- `CLAUDECODE` env var is removed on `tmux new-session`, not on `tmux attach`

tmux binary: resolved at runtime by checking `/opt/homebrew/bin/tmux`, then `/usr/local/bin/tmux`, then `tmux` on `PATH`. Session naming: `ctrl-{uuid}`.

Affected files:
- `server/src/tmux.rs` — tmux binary interactions
- `server/src/pty_manager.rs` — `spawn_session`, `close_session`, `attach_tmux_session`
- `server/src/main.rs` — axum entry point; schedulers + status_socket start here

## Shell Environment Inheritance (macOS GUI)

macOS GUI apps inherit a minimal launchd environment missing `.zshrc` vars. `shell_env::inherit_shell_env()` resolves the user's full shell env at startup and applies it to the process. Must run before any threads (`set_var` is not thread-safe). For tmux, all process env vars are passed via `-e` flags in `build_create_args` because tmux sessions inherit the **server's** environment, not the client's.

Affected files: `server/src/shell_env.rs`, `server/src/main.rs`, `server/src/tmux.rs`

## CLAUDECODE Environment Variable

Claude Code sets a `CLAUDECODE` env var to detect nested sessions. All `Command::new("claude")` calls and PTY `CommandBuilder` spawns must include `.env_remove("CLAUDECODE")` to prevent "cannot be launched inside another Claude Code session" errors.

Affected locations:
- `server/src/tmux.rs` — `create_session` (removes CLAUDECODE for tmux-backed sessions)
- `server/src/pty_manager.rs` — `spawn_command` (removes CLAUDECODE for direct commands)
- `server/src/config.rs` — `check_claude_cli_status`, `generate_names_via_cli`
- `server/src/maintainer.rs` — `run_health_check` (removes CLAUDECODE for health check subprocess)

## Session Status Detection via Hooks

Session status (idle/working/exited) is detected using Claude Code hooks, not PTY output heuristics.

**How it works:**
1. On app startup, a Unix domain socket listener starts at `/tmp/the-controller.sock`.
2. When spawning Claude sessions, `--settings` is passed with hook config for `UserPromptSubmit` (→ working), `Stop` (→ idle), and `Notification[idle_prompt]` (→ idle).
3. Hook commands send `status:session-id` to the socket via `nc -U`.
4. The socket listener emits `session-status-hook:<session-id>` events over the WebSocket broadcaster.
5. PTY EOF (`session-status-changed`) still handles the "exited" state.

**Key files:**
- `server/src/status_socket.rs` — socket listener, message parsing, hook JSON generation
- `server/src/tmux.rs` — passes `--settings` and `THE_CONTROLLER_SESSION_ID` env var
- `server/src/pty_manager.rs` — same for direct (non-tmux) sessions
- `src/lib/Sidebar.svelte` — listens for `session-status-hook` events

**Edge cases:**
- Hook commands use `nc -w 2` + `; true` to avoid blocking Claude Code (`timeout` is not available on macOS)
- Stale socket files are cleaned up on startup
- Reattached tmux sessions default to "idle" until the next hook fires
