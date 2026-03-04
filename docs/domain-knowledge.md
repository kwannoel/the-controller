# Domain Knowledge

Lessons learned during development. Check this before making changes.

## Tauri v2: Synchronous Commands Block the Webview

**Problem:** Tauri commands defined as `pub fn` (synchronous) run on the **main thread**. If the command does anything slow (subprocess calls, file I/O, network), it freezes the entire webview — no rendering, no animations, no user interaction.

**Symptom:** UI appears "stuck" even though JavaScript has already updated the state. The browser can't paint because the main thread is blocked by the Rust command.

**Fix:** Make slow commands `pub async fn` and use `tokio::task::spawn_blocking` for CPU/IO-bound work:

```rust
// BAD: blocks main thread
#[tauri::command]
pub fn slow_command() -> Result<String, String> {
    let result = expensive_operation(); // freezes webview
    Ok(result)
}

// GOOD: runs on background thread
#[tauri::command]
pub async fn slow_command() -> Result<String, String> {
    let result = tokio::task::spawn_blocking(|| expensive_operation())
        .await
        .map_err(|e| format!("Task failed: {}", e))?;
    Ok(result)
}
```

**Rule of thumb:** Any command that shells out (`Command::new(...)`) or does significant I/O must be async + spawn_blocking.

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

tmux binary: `/opt/homebrew/bin/tmux`. Session naming: `ctrl-{uuid}`.

Affected files:
- `src-tauri/src/tmux.rs` — tmux binary interactions
- `src-tauri/src/pty_manager.rs` — `spawn_session`, `close_session`, `attach_tmux_session`
- `src-tauri/src/lib.rs` — exit handler that kills tmux sessions

## CLAUDECODE Environment Variable

Claude Code sets a `CLAUDECODE` env var to detect nested sessions. All `Command::new("claude")` calls and PTY `CommandBuilder` spawns must include `.env_remove("CLAUDECODE")` to prevent "cannot be launched inside another Claude Code session" errors.

Affected locations:
- `src-tauri/src/tmux.rs` — `create_session` (removes CLAUDECODE for tmux-backed sessions)
- `src-tauri/src/pty_manager.rs` — `spawn_command` (removes CLAUDECODE for direct commands)
- `src-tauri/src/config.rs` — `check_claude_cli_status`, `generate_names_via_cli`
