# Maintainer Check Threading Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Prevent manual maintainer checks from blocking Tokio async runtime threads.

**Architecture:** Keep `run_maintainer_check` synchronous, but move the manual command boundary onto Tokio's blocking pool with a small helper in `commands.rs`. Add a closure-injected helper for a behavioral regression test that verifies the work executes off the runtime thread.

**Tech Stack:** Rust, Tokio, Tauri v2 test helpers

---

### Task 1: Add the regression test

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Test: `src-tauri/src/commands.rs`

**Step 1: Write the failing test**

Add a unit test that captures the async runtime thread ID, invokes the helper with a closure, and asserts the closure runs on a different thread.

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test test_trigger_maintainer_check_runs_on_blocking_thread`

Expected: FAIL because the helper still executes inline on the runtime thread.

### Task 2: Implement the minimal fix

**Files:**
- Modify: `src-tauri/src/commands.rs`

**Step 1: Add an async helper**

Create a helper that accepts owned maintainer-check inputs and uses `tokio::task::spawn_blocking` to execute the blocking runner.

**Step 2: Route the command through the helper**

Keep the existing event emission and storage behavior intact while replacing the direct call in `trigger_maintainer_check`.

**Step 3: Re-run the targeted test**

Run: `cd src-tauri && cargo test test_trigger_maintainer_check_runs_on_blocking_thread`

Expected: PASS

### Task 3: Verify broader behavior

**Files:**
- Modify: `src-tauri/src/commands.rs`

**Step 1: Run the full Rust test suite**

Run: `cd src-tauri && cargo test`

Expected: PASS

**Step 2: Review the diff**

Confirm the only behavior change is offloading the blocking maintainer run from the async runtime thread to Tokio's blocking pool.

**Step 3: Commit**

Run:

```bash
git add docs/plans/2026-03-10-maintainer-check-threading-design.md docs/plans/2026-03-10-maintainer-check-threading.md src-tauri/src/commands.rs
git commit
```

Commit message body must end with:

```text
Contributed-by: auto-worker
```
