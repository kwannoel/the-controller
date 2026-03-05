# Session Status Hooks Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace unreliable pty-output debounce with Claude Code hooks communicating via Unix domain socket for precise session status detection.

**Architecture:** Socket listener runs as a background tokio task on app startup. When spawning Claude Code sessions, we pass `--settings` with hook config that sends status messages to the socket. Frontend listens for hook-based status events instead of debouncing pty-output.

**Tech Stack:** Rust (std::os::unix::net, tokio), Svelte 5, Tauri v2 events

---

### Task 1: Add Unix domain socket listener module

**Files:**
- Create: `src-tauri/src/status_socket.rs`
- Modify: `src-tauri/src/lib.rs:1-10` (add `pub mod status_socket;`)

**Step 1: Write the test**

In `src-tauri/src/status_socket.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    #[test]
    fn test_parse_status_message_working() {
        let id = uuid::Uuid::new_v4();
        let msg = format!("working:{}", id);
        let (status, parsed_id) = parse_status_message(&msg).unwrap();
        assert_eq!(status, "working");
        assert_eq!(parsed_id, id);
    }

    #[test]
    fn test_parse_status_message_idle() {
        let id = uuid::Uuid::new_v4();
        let msg = format!("idle:{}", id);
        let (status, parsed_id) = parse_status_message(&msg).unwrap();
        assert_eq!(status, "idle");
        assert_eq!(parsed_id, id);
    }

    #[test]
    fn test_parse_status_message_invalid_format() {
        assert!(parse_status_message("garbage").is_none());
        assert!(parse_status_message("working:not-a-uuid").is_none());
        assert!(parse_status_message("unknown:550e8400-e29b-41d4-a716-446655440000").is_none());
        assert!(parse_status_message("").is_none());
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test status_socket`
Expected: FAIL — module and function don't exist.

**Step 3: Write implementation**

In `src-tauri/src/status_socket.rs`:

```rust
use std::io::{BufRead, BufReader};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

const SOCKET_PATH: &str = "/tmp/the-controller.sock";

/// Parse a "status:uuid" message. Returns (status, session_id) or None.
pub fn parse_status_message(msg: &str) -> Option<(&str, Uuid)> {
    let (status, id_str) = msg.split_once(':')?;
    if status != "working" && status != "idle" {
        return None;
    }
    let id = Uuid::parse_str(id_str).ok()?;
    Some((status, id))
}

/// Clean up a stale socket file if no other instance is listening.
fn cleanup_stale_socket(path: &Path) {
    if path.exists() {
        match UnixStream::connect(path) {
            Ok(_) => {
                eprintln!("Warning: another instance of The Controller appears to be running");
            }
            Err(_) => {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

/// Start the Unix domain socket listener. Call this once on app startup,
/// before any sessions are spawned. Runs in a background thread.
pub fn start_listener(app_handle: AppHandle) {
    let path = PathBuf::from(SOCKET_PATH);
    cleanup_stale_socket(&path);

    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind status socket at {}: {}", SOCKET_PATH, e);
            return;
        }
    };

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    handle_connection(stream, &app_handle);
                }
                Err(e) => {
                    eprintln!("Status socket accept error: {}", e);
                }
            }
        }
    });
}

fn handle_connection(stream: UnixStream, app_handle: &AppHandle) {
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = line.trim();
        if let Some((status, session_id)) = parse_status_message(trimmed) {
            let event_name = format!("session-status-hook:{}", session_id);
            let _ = app_handle.emit(&event_name, status);
        }
    }
}

/// Remove the socket file. Call on app shutdown.
pub fn cleanup() {
    let _ = std::fs::remove_file(SOCKET_PATH);
}

/// Get the socket path (for building hook commands).
pub fn socket_path() -> &'static str {
    SOCKET_PATH
}
```

Add to `src-tauri/src/lib.rs` (line 1-10 area, with the other `pub mod` declarations):

```rust
pub mod status_socket;
```

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test status_socket`
Expected: All 3 tests PASS.

**Step 5: Commit**

```bash
git add src-tauri/src/status_socket.rs src-tauri/src/lib.rs
git commit -m "feat: add status socket listener module (#28)"
```

---

### Task 2: Start socket listener on app startup, clean up on shutdown

**Files:**
- Modify: `src-tauri/src/lib.rs:12-68`

**Step 1: Start listener in the Tauri setup hook, clean up on exit**

In `src-tauri/src/lib.rs`, add a `.setup()` call and update the exit handler:

```rust
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(state::AppState::new())
        .setup(|app| {
            status_socket::start_listener(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // ... existing handlers unchanged ...
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                status_socket::cleanup();
                if cfg!(not(debug_assertions)) {
                    if let Some(state) = app_handle.try_state::<state::AppState>() {
                        if let Ok(mut pty_manager) = state.pty_manager.lock() {
                            let ids = pty_manager.session_ids();
                            for id in ids {
                                let _ = pty_manager.close_session(id);
                            }
                        }
                    }
                }
            }
        });
}
```

**Step 2: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: Compiles successfully.

**Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: start status socket on app startup, cleanup on exit (#28)"
```

---

### Task 3: Pass hook settings when spawning Claude sessions

**Files:**
- Modify: `src-tauri/src/tmux.rs:28-62` (`create_session`)
- Modify: `src-tauri/src/pty_manager.rs:60-140` (`spawn_direct_session`)

**Step 1: Write test for hook settings JSON generation**

Add to `src-tauri/src/status_socket.rs` tests:

```rust
#[test]
fn test_hook_settings_json_is_valid() {
    let id = uuid::Uuid::new_v4();
    let json = super::hook_settings_json(id);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let hooks = parsed.get("hooks").unwrap();
    assert!(hooks.get("UserPromptSubmit").is_some());
    assert!(hooks.get("Stop").is_some());
    assert!(hooks.get("Notification").is_some());
}
```

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test test_hook_settings`
Expected: FAIL — `hook_settings_json` doesn't exist.

**Step 3: Add `hook_settings_json` to `status_socket.rs`**

```rust
/// Generate the `--settings` JSON string with hooks that report status
/// back to The Controller via the Unix domain socket.
pub fn hook_settings_json(session_id: Uuid) -> String {
    let cmd = format!(
        "timeout 2 bash -c 'echo \"{{status}}:$THE_CONTROLLER_SESSION_ID\" | nc -U {}' 2>/dev/null; true",
        SOCKET_PATH
    );
    let working_cmd = cmd.replace("{status}", "working");
    let idle_cmd = cmd.replace("{status}", "idle");

    serde_json::json!({
        "hooks": {
            "UserPromptSubmit": [{
                "type": "command",
                "command": working_cmd
            }],
            "Stop": [{
                "type": "command",
                "command": idle_cmd
            }],
            "Notification": [{
                "matcher": "idle_prompt",
                "hooks": [{
                    "type": "command",
                    "command": idle_cmd
                }]
            }]
        }
    }).to_string()
}
```

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test test_hook_settings`
Expected: PASS.

**Step 5: Pass `--settings` and env var in `TmuxManager::create_session`**

In `src-tauri/src/tmux.rs`, modify `create_session` to accept and pass the settings JSON:

```rust
pub fn create_session(
    session_id: Uuid,
    working_dir: &str,
    command: &str,
    continue_session: bool,
) -> Result<(), String> {
    let name = Self::session_name(session_id);
    let settings_json = crate::status_socket::hook_settings_json(session_id);

    let mut args = vec![
        "new-session", "-d", "-s", &name, "-c", working_dir, "-x", "80", "-y", "24", command,
    ];
    // Append claude-specific flags after the command name
    if continue_session {
        args.push("--continue");
    }
    args.push("--settings");
    args.push(&settings_json);

    let output = Command::new(TMUX_BIN)
        .args(&args)
        .env_remove("CLAUDECODE")
        .env("THE_CONTROLLER_SESSION_ID", session_id.to_string())
        .output()
        .map_err(|e| format!("failed to run tmux: {}", e))?;
    // ... rest unchanged ...
```

**Step 6: Pass `--settings` and env var in `PtyManager::spawn_direct_session`**

In `src-tauri/src/pty_manager.rs`, modify `spawn_direct_session`:

```rust
fn spawn_direct_session(
    &mut self,
    session_id: Uuid,
    working_dir: &str,
    command: &str,
    app_handle: AppHandle,
) -> Result<(), String> {
    // ... pty_system and pair setup unchanged ...

    let mut cmd = CommandBuilder::new(command);
    cmd.cwd(working_dir);
    cmd.env_remove("CLAUDECODE");
    cmd.env("THE_CONTROLLER_SESSION_ID", session_id.to_string());

    // Only pass --settings for claude sessions (not codex or other commands)
    if command == "claude" {
        let settings_json = crate::status_socket::hook_settings_json(session_id);
        cmd.arg("--settings");
        cmd.arg(settings_json);
    }

    // ... rest unchanged ...
```

**Step 7: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: Compiles successfully.

**Step 8: Commit**

```bash
git add src-tauri/src/status_socket.rs src-tauri/src/tmux.rs src-tauri/src/pty_manager.rs
git commit -m "feat: pass hook settings to Claude sessions for status reporting (#28)"
```

---

### Task 4: Update frontend to use hook-based status events

**Files:**
- Modify: `src/lib/Sidebar.svelte:53-55,240-292,337,418,470-471,565-568,852-856`

**Step 1: Replace pty-output debounce with hook listener**

In `src/lib/Sidebar.svelte`:

1. **Remove** the `idleTimers` map and `IDLE_TIMEOUT_MS` constant (lines 54-55).

2. **Remove** the `resetIdleTimer` function (lines 248-262).

3. **Replace** the `$effect` block (lines 264-292) that sets up `pty-output` and `session-status-changed` listeners with:

```typescript
$effect(() => {
    const unlisteners: (() => void)[] = [];
    let cancelled = false;

    for (const project of projectList) {
      for (const session of project.sessions) {
        // Hook-based status: listen for precise idle/working from Claude Code hooks
        listen<string>(`session-status-hook:${session.id}`, (event) => {
          const status = event.payload as SessionStatus;
          if (status === "working" || status === "idle") {
            markSession(session.id, status);
          }
        }).then(unlisten => { if (!cancelled) unlisteners.push(unlisten); else unlisten(); });

        // PTY exit: session process has ended
        listen<string>(`session-status-changed:${session.id}`, () => {
          markSession(session.id, "exited");
        }).then(unlisten => { if (!cancelled) unlisteners.push(unlisten); else unlisten(); });
      }
    }

    return () => {
      cancelled = true;
      unlisteners.forEach(fn => fn());
    };
  });
```

4. **Remove** all `idleTimers` references in `closeSession` (lines 364-366), `archiveSession` (lines 394-396), and anywhere else they appear.

5. **Keep** `markSession(sessionId, "working")` calls in `createSession` (line 337) and `unarchiveSession` (line 418) — these set the initial status before hooks fire.

**Step 2: Verify it compiles**

Run: `npm run check` (or `npx svelte-check`)
Expected: No errors.

**Step 3: Commit**

```bash
git add src/lib/Sidebar.svelte
git commit -m "feat: replace pty-output debounce with hook-based status events (#28)"
```

---

### Task 5: Manual integration test

**Step 1: Run the app**

Run: `npm run tauri dev`

**Step 2: Verify socket exists**

Run: `ls -la /tmp/the-controller.sock`
Expected: Socket file exists.

**Step 3: Create a session and verify status transitions**

1. Create a new project and session.
2. Observe the status dot is **yellow** (working) when Claude is processing.
3. Wait for Claude to finish — status dot should turn **green** (idle).
4. Type a message — status dot should turn **yellow** (working) again.
5. Close the session — status dot should show **gray** (exited circle).

**Step 4: Test edge cases**

1. **Tab switching**: Switch between sessions — idle sessions should stay green (not flash yellow).
2. **Long thinking**: Give Claude a complex task — status should stay yellow throughout, not flash green during pauses.
3. **App restart**: Quit and relaunch — reattached sessions should default to idle (green).

**Step 5: Verify socket cleanup**

1. Quit the app.
2. Run: `ls /tmp/the-controller.sock`
3. Expected: Socket file should not exist.

**Step 6: Commit**

No code changes expected. If fixes are needed, commit them.

---

### Task 6: Update domain knowledge docs

**Files:**
- Modify: `docs/domain-knowledge.md`

**Step 1: Add section about session status hooks**

Append to `docs/domain-knowledge.md`:

```markdown

## Session Status Detection via Hooks

Session status (idle/working/exited) is detected using Claude Code hooks, not PTY output heuristics.

**How it works:**
1. On app startup, a Unix domain socket listener starts at `/tmp/the-controller.sock`.
2. When spawning Claude sessions, `--settings` is passed with hook config for `UserPromptSubmit` (→ working), `Stop` (→ idle), and `Notification[idle_prompt]` (→ idle).
3. Hook commands send `status:session-id` to the socket via `nc -U`.
4. The socket listener emits `session-status-hook:<session-id>` Tauri events.
5. PTY EOF (`session-status-changed`) still handles the "exited" state.

**Key files:**
- `src-tauri/src/status_socket.rs` — socket listener, message parsing, hook JSON generation
- `src-tauri/src/tmux.rs` — passes `--settings` and `THE_CONTROLLER_SESSION_ID` env var
- `src-tauri/src/pty_manager.rs` — same for direct (non-tmux) sessions
- `src/lib/Sidebar.svelte` — listens for `session-status-hook` events

**Edge cases:**
- Hook commands use `timeout 2` + `; true` to avoid blocking Claude Code
- Stale socket files are cleaned up on startup
- Reattached tmux sessions default to "idle" until the next hook fires
```

**Step 2: Commit**

```bash
git add docs/domain-knowledge.md
git commit -m "docs: add session status hooks to domain knowledge (#28)"
```
