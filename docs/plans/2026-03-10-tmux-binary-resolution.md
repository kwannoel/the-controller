# Tmux Binary Resolution Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Make tmux-backed sessions work on Intel Macs by resolving tmux from shared runtime logic instead of hardcoded Apple Silicon paths.

**Architecture:** Centralize tmux binary resolution in `TmuxManager`, use it for all tmux subprocess calls, and route PTY attach through the same resolver. Prove the bug and the fix with regression tests that fail if either tmux availability checks or attach logic fall back to hardcoded paths again.

**Tech Stack:** Rust, portable-pty, cargo test

---

### Task 1: Add Resolution Regression Coverage

**Files:**

- Modify: `src-tauri/src/tmux.rs`
- Test: `src-tauri/src/tmux.rs`

**Step 1: Write the failing test**

Add a test that creates a temporary executable named `tmux`, prepends its directory to `PATH`, and asserts `TmuxManager::is_available()` returns `true`.

**Step 2: Run test to verify it fails**

Run:

- `cd src-tauri && cargo test test_is_available_finds_tmux_on_path -- --test-threads=1`

Expected: FAIL because the current implementation only checks `/opt/homebrew/bin/tmux`.

**Step 3: Write minimal implementation**

Add a shared tmux resolver that checks `/opt/homebrew/bin/tmux`, `/usr/local/bin/tmux`, then `PATH`, and use it from `TmuxManager`.

**Step 4: Run test to verify it passes**

Run the same command and confirm PASS.

**Step 5: Commit**

Commit after broader verification and review are complete.

### Task 2: Cover PTY Attach And Verify

**Files:**

- Modify: `src-tauri/src/pty_manager.rs`
- Modify: `src-tauri/src/tmux.rs`
- Modify: `docs/domain-knowledge.md`

**Step 1: Write the failing test**

Add a PTY-manager regression test that supplies a fake tmux executable via `PATH`, calls `spawn_session`, and asserts the tmux-backed attach path succeeds.

**Step 2: Run test to verify it fails**

Run:

- `cd src-tauri && cargo test test_spawn_session_attaches_with_tmux_from_path -- --test-threads=1`

Expected: FAIL because `attach_tmux_session` still hardcodes `/opt/homebrew/bin/tmux`.

**Step 3: Write minimal implementation**

Use the shared tmux resolver when building the attach command and update docs to describe runtime resolution instead of a fixed path.

**Step 4: Run test to verify it passes**

Run the same test command and confirm PASS.

**Step 5: Commit**

Use a commit message body that includes:

```text
fix: resolve tmux binary at runtime

closes #246

Contributed-by: auto-worker
```
