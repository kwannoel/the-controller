# Session Status Detection via Claude Code Hooks

## Problem

The current session status detection uses a 3-second debounce on `pty-output` events to infer whether Claude Code is idle or working. This is unreliable:

- **False idle**: Claude is thinking/processing without streaming output for >3s, so it transitions to "idle" (green) even though it's still working.
- **False working**: Switching tabs triggers xterm.js resize/reflow, which emits `pty-output` events, marking an idle session as "working" (yellow).

## Solution

Use Claude Code's built-in hooks system to get precise state transitions, communicated to The Controller via a Unix domain socket.

## Architecture

```
Claude Code (in PTY)
  |-- UserPromptSubmit hook --> writes "working:<session-id>" to socket
  |-- Stop hook             --> writes "idle:<session-id>" to socket
  |-- Notification(idle_prompt) --> writes "idle:<session-id>" to socket

The Controller (Rust backend)
  |-- UnixListener on /tmp/the-controller.sock
       |-- Parse "status:session-id"
       |-- Emit Tauri event "session-status-hook:<session-id>"

Frontend (Svelte)
  |-- Listen for "session-status-hook:<session-id>" --> update sessionStatuses store
  |-- Keep "session-status-changed" listener for "exited" state (PTY EOF)
```

## Hook Configuration

When spawning a Claude Code session, The Controller:

1. Sets env var `THE_CONTROLLER_SESSION_ID=<uuid>` on the PTY command.
2. Passes `--settings <json>` with hook configuration:

```json
{
  "hooks": {
    "UserPromptSubmit": [{
      "type": "command",
      "command": "echo \"working:$THE_CONTROLLER_SESSION_ID\" | nc -U -w 2 /tmp/the-controller.sock 2>/dev/null; true"
    }],
    "Stop": [{
      "type": "command",
      "command": "echo \"idle:$THE_CONTROLLER_SESSION_ID\" | nc -U -w 2 /tmp/the-controller.sock 2>/dev/null; true"
    }],
    "Notification": [{
      "matcher": "idle_prompt",
      "hooks": [{
        "type": "command",
        "command": "echo \"idle:$THE_CONTROLLER_SESSION_ID\" | nc -U -w 2 /tmp/the-controller.sock 2>/dev/null; true"
      }]
    }]
  }
}
```

The `nc -w 2` flag sets a 2-second idle timeout, preventing the hook from blocking Claude Code if the socket is unavailable. The `; true` ensures exit code 0 so Claude Code doesn't treat it as a hook failure. Note: `timeout` is not available on macOS by default, so we use `nc -w` instead.

## Socket Listener

- Started once on app startup in a background tokio task.
- Listens on `/tmp/the-controller.sock`.
- On startup: attempt connect to existing socket. If connection refused, unlink stale file. If connection succeeds, another instance is running — warn user.
- Each incoming connection: read line, parse `status:session-id`, emit Tauri event `session-status-hook:<session-id>`.
- On app shutdown: unlink the socket file.

## Frontend Changes

- Remove the `pty-output` debounce logic for status detection from `Sidebar.svelte`.
- Listen for `session-status-hook:<session-id>` events.
- Keep `session-status-changed` listener for the "exited" state (PTY EOF).
- Default new sessions to "working" (they boot up immediately).

## Session Spawn Changes

In `pty_manager.rs` (or `tmux.rs`), when spawning the `claude` command:

1. Add `THE_CONTROLLER_SESSION_ID` env var to the command environment.
2. Generate the hooks settings JSON.
3. Append `--settings <json>` to the `claude` command arguments.

## Edge Cases

### 1. Hook command blocks Claude Code (Critical)

If `nc -U` hangs (e.g., socket file exists but nobody is accepting connections), the hook blocks Claude Code from proceeding.

**Mitigation:** All hook commands are wrapped with `timeout 2` and `; true`. If the socket is unavailable, the hook exits in at most 2 seconds with no error propagated to Claude Code.

### 2. Stale socket file on crash

If The Controller crashes, `/tmp/the-controller.sock` remains on disk. On restart, `bind()` fails with "address already in use".

**Mitigation:** On startup, try to connect to the existing socket:
- Connection refused: stale file, unlink it and proceed.
- Connection succeeds: another instance is running, warn the user and exit or take over.

### 3. Multiple Controller instances

Two instances would fight over the socket path.

**Mitigation:** Always use `/tmp/the-controller.sock`. Only one instance should run at a time. Detect via the stale socket check above. If another instance is live, warn the user.

### 4. Session exits without Stop hook firing

If the PTY is killed or Claude Code crashes, the `Stop` hook never fires. The session would remain in "working" state indefinitely.

**Mitigation:** Already handled. The existing `session-status-changed` event (PTY reader EOF) fires when the process exits, and the frontend transitions the session to "exited". This takes precedence over hook-based status.

### 5. Unknown status on tmux reattachment

When The Controller restarts and reattaches to existing tmux sessions, the hooks are still configured and `THE_CONTROLLER_SESSION_ID` persists in the tmux environment. However, we don't know the current state — Claude Code could be idle or working.

**Mitigation:** Default reattached sessions to "idle". If Claude Code is actively working, a `Stop` hook will fire when it finishes, confirming the idle state. If the user submits input, a `UserPromptSubmit` hook fires and marks it as "working". The brief period of potentially wrong status on reattach is acceptable.

### 6. Malformed messages on the socket

Random or malformed data could be sent to the socket.

**Mitigation:** Validate that each message matches the format `(working|idle):<valid-uuid>`. Discard anything that doesn't match. Log a warning for debugging.

### 7. Socket listener not ready before first hook fires

If a session is spawned very quickly after app startup, the first hook could fire before the socket listener is bound.

**Mitigation:** Start the socket listener before spawning any sessions. The socket bind is synchronous and fast — it will be ready before any `create_session` call is possible from the frontend.

### 8. Rapid state transitions

User submits a prompt and gets an instant response — `UserPromptSubmit` (working) and `Stop` (idle) fire in quick succession on separate socket connections.

**Mitigation:** Each connection is handled sequentially or with ordered processing. The final state ("idle") is correct. Even if processing order is briefly swapped, the next event corrects it. No special handling needed.
