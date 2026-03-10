# PTY Child Handle Retention Design

## Problem

`PtyManager` spawns direct PTY sessions, tmux attachment PTYs, and short-lived direct commands through `portable-pty`, but each spawn path drops the returned child handle immediately. That leaves `close_session` with no reliable way to terminate or reap the spawned process. If the child ignores `SIGHUP`, it can outlive the session and keep resources such as a worktree open.

## Constraints

- Keep the fix local to PTY lifecycle management in `src-tauri/src/pty_manager.rs`.
- Preserve existing tmux behavior: closing a tmux-backed session must still kill the tmux session itself, not just the local attachment process.
- Add an automated regression that proves the bug and the fix, rather than relying on a structural assertion.

## Options Considered

### 1. Keep relying on PTY teardown

Rejected. Closing the PTY master depends on child signal handling and does not guarantee termination or reaping.

### 2. Store the spawned child handle in `PtySession`

Chosen. This gives `close_session` ownership of the child lifecycle so it can terminate and reap the process explicitly before dropping the PTY session state.

### 3. Add a separate background reaper

Rejected. It adds complexity and shared-state coordination for a bug that can be fixed directly where the session is owned and closed.

## Design

- Extend `PtySession` with a `child: Box<dyn portable_pty::Child + Send + Sync>` field.
- Update `spawn_direct_session`, `attach_tmux_session`, and `spawn_command` to store the returned child handle in the session record.
- In `close_session`, call `try_wait()` first. If the child is still running, call `kill()` and then `wait()` to reap it before the session is dropped.
- Preserve the existing tmux cleanup path by still calling `TmuxManager::kill_session(session_id)` for tmux-backed sessions after the local child is handled.

## Validation

- Add a Unix regression test that starts `/bin/sh`, writes its PID to a file, installs `trap '' HUP`, loops forever, and then verifies `close_session` makes the process disappear.
- Run the regression test red before the fix.
- Run the regression test green after the fix.
- Run the PTY manager tests and the full Rust test suite after the implementation.
