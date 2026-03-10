# PTY Child Handle Retention Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Retain PTY child handles so `close_session` can explicitly terminate and reap spawned processes.

**Architecture:** Extend `PtySession` to own the `portable-pty` child handle, update each spawn path to keep that handle alive, and add a regression test that proves `close_session` terminates a child that ignores `SIGHUP`. The close path should reap the child before any tmux-specific cleanup runs.

**Tech Stack:** Rust, portable-pty, Cargo tests

---

### Task 1: Reproduce the lifecycle bug with a failing regression test

**Files:**
- Modify: `src-tauri/src/pty_manager.rs`
- Test: `src-tauri/src/pty_manager.rs`

**Step 1: Write the failing test**

Add a Unix-only unit test that:
- Spawns `/bin/sh` through `spawn_command`
- Writes the shell PID to a temp file
- Installs `trap '' HUP`
- Loops forever
- Calls `close_session`
- Asserts the PID exits within a short timeout

**Step 2: Run test to verify it fails**

Run: `cargo test close_session_kills_direct_child_that_ignores_sighup -- --nocapture`
Expected: FAIL because the shell process survives `close_session`

**Step 3: Commit**

Do not commit yet. Continue to Task 2 once the failing behavior is confirmed.

### Task 2: Retain the child handle and close it explicitly

**Files:**
- Modify: `src-tauri/src/pty_manager.rs`
- Test: `src-tauri/src/pty_manager.rs`

**Step 1: Write minimal implementation**

- Add `child` to `PtySession`
- Store the child handle in all three spawn paths
- In `close_session`, `try_wait()` first, then `kill()` and `wait()` if the child is still alive
- Preserve existing tmux cleanup

**Step 2: Run targeted tests to verify they pass**

Run: `cargo test pty_manager::tests`
Expected: PASS

**Step 3: Run full verification**

Run: `cargo test`
Expected: PASS

**Step 4: Commit**

```bash
git add docs/plans/2026-03-10-pty-child-handle-design.md docs/plans/2026-03-10-pty-child-handle.md src-tauri/src/pty_manager.rs
git commit
```
