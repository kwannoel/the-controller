# Auto-Worker Restart Recovery Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Make auto-worker startup restart-safe so dev restarts or overlapping app instances do not spawn duplicate workers for the same issue.

**Architecture:** Restore active auto-worker sessions from persisted project state plus tmux liveness before the scheduler poll loop starts. Clean only truly stale worker sessions and labels, then let the existing scheduler loop continue managing restored sessions and new issue selection.

**Tech Stack:** Rust, Tauri v2, tmux, cargo test

---

### Task 1: Document and encode the startup restoration decision

**Files:**
- Create: `docs/plans/2026-03-10-auto-worker-restart-recovery-design.md`
- Modify: `src-tauri/src/auto_worker.rs`
- Test: `src-tauri/src/auto_worker.rs`

**Step 1: Write the failing test**

Add a unit test in `src-tauri/src/auto_worker.rs` for a helper that evaluates persisted worker sessions on startup and distinguishes:
- one live worker to restore;
- one stale worker to clean.

The test should model duplicate persisted worker sessions for one project and assert only one survives restoration.

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test auto_worker::tests::startup_restoration_keeps_one_live_worker_and_cleans_stale_duplicates`

Expected: FAIL because the helper or restoration behavior does not exist yet.

**Step 3: Write minimal implementation**

In `src-tauri/src/auto_worker.rs`:
- add a small restoration model/helper for startup session evaluation;
- make the helper deterministic so the unit test can cover the duplicate-selection policy without tmux side effects.

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test auto_worker::tests::startup_restoration_keeps_one_live_worker_and_cleans_stale_duplicates`

Expected: PASS.

**Step 5: Commit**

```bash
git add src-tauri/src/auto_worker.rs docs/plans/2026-03-10-auto-worker-restart-recovery-design.md docs/plans/2026-03-10-auto-worker-restart-recovery.md
git commit -m "test: cover auto-worker startup restoration"
```

### Task 2: Restore live auto-worker sessions on startup

**Files:**
- Modify: `src-tauri/src/auto_worker.rs`
- Modify: `src-tauri/src/tmux.rs`

**Step 1: Write the failing test**

Add or extend a unit test around the startup helper so it proves the current startup cleanup would incorrectly drop a live worker session that still has a tmux session.

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test auto_worker::tests::startup_restoration_preserves_live_tmux_owned_issue`

Expected: FAIL against the old startup assumptions.

**Step 3: Write minimal implementation**

Implement startup restoration in `AutoWorkerScheduler::start`:
- discover persisted `auto_worker_session` records for enabled projects;
- use tmux session existence to decide whether each worker is live;
- rebuild `active_sessions` from the surviving live records;
- keep only one restored worker per project.

Add any small tmux helper you need, but keep it narrow.

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test auto_worker::tests::startup_restoration_preserves_live_tmux_owned_issue`

Expected: PASS.

**Step 5: Commit**

```bash
git add src-tauri/src/auto_worker.rs src-tauri/src/tmux.rs
git commit -m "fix: restore live auto-worker sessions on startup"
```

### Task 3: Clean stale worker sessions and labels without touching live work

**Files:**
- Modify: `src-tauri/src/auto_worker.rs`
- Test: `src-tauri/src/auto_worker.rs`

**Step 1: Write the failing test**

Add a unit test that feeds startup cleanup a mix of:
- restored live worker ownership;
- stale persisted worker sessions;
- orphaned `in-progress` labels.

Assert cleanup removes only stale/orphaned labels and retains the live worker label.

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test auto_worker::tests::startup_cleanup_keeps_live_in_progress_labels`

Expected: FAIL before the cleanup logic is restoration-aware.

**Step 3: Write minimal implementation**

Update startup cleanup so:
- stale persisted worker sessions are removed and cleaned;
- stale labels are removed only when they are not owned by restored live workers.

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test auto_worker::tests::startup_cleanup_keeps_live_in_progress_labels`

Expected: PASS.

**Step 5: Commit**

```bash
git add src-tauri/src/auto_worker.rs
git commit -m "fix: avoid clearing live auto-worker state on restart"
```

### Task 4: Verify the scheduler behavior and clean the live state

**Files:**
- Modify: `src-tauri/src/auto_worker.rs` if needed

**Step 1: Run focused backend verification**

Run: `cd src-tauri && cargo test auto_worker`

Expected: PASS.

**Step 2: Run broader regression coverage if the helper touches shared startup code**

Run: `cd src-tauri && cargo test`

Expected: PASS, or document any unrelated existing failures.

**Step 3: Clean the live duplicate worker state**

Use tmux and GitHub CLI to:
- kill stale duplicate worker sessions for `#328`;
- remove stale persisted worker session entries left behind from completed runs;
- preserve only the current live worker for `#327`;
- remove any stale `in-progress` label from closed issues.

**Step 4: Verify runtime state**

Confirm:
- one live auto-worker session remains for `the-controller`;
- issue `#328` is closed and no longer labeled `in-progress`;
- issue `#327` has at most one active worker owner.

**Step 5: Commit**

```bash
git add src-tauri/src/auto_worker.rs
git commit -m "fix: recover auto-worker state safely across restarts"
```
