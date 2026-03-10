# Merge Session Branch Timeout Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Stop `merge_session_branch` from polling forever when Claude never resolves merge conflicts.

**Architecture:** Keep the existing merge flow, but replace the unbounded rebase-resolution wait with a bounded polling helper that returns a clear timeout error. Match the existing `stage_session_inplace` behavior rather than introducing a new cancellation system in this fix.

**Tech Stack:** Rust, Tauri v2 commands, Tokio async runtime, git2-backed repository helpers

---

## Definition

The task is to fix issue #249 by ensuring `merge_session_branch` no longer hangs indefinitely after a conflicted rebase. The command should still prompt Claude to resolve conflicts and poll for completion, but it must stop after a defined deadline and return an actionable error to the frontend.

## Constraints

- The issue asks for timeout or cancellation; this fix will implement timeout because it solves the hang with the smallest change and matches an existing command in the same file.
- The command already emits `"merge-status"` progress updates and focuses the active terminal from the frontend; preserve that flow.
- Follow TDD: the timeout behavior must be covered by a failing test first.
- Avoid wide refactors of merge, PR creation, or frontend state unless the test proves they are necessary.

## Validation

- Add a test that exercises the merge rebase wait behavior with a condition that never resolves and verifies it returns a timeout error instead of waiting forever.
- Run the new targeted Rust test and watch it fail before the implementation.
- Run the same targeted test again after the fix and verify it passes.
- Run a broader Rust test command for `commands::tests` to check for regressions in the command module.

## Approaches Considered

1. Add a timeout to the existing polling loop.
   Recommendation: minimal change, directly fixes the bug, and matches `stage_session_inplace`.
2. Add frontend-triggered cancellation with a `CancellationToken`.
   More flexible, but it requires new shared state and UI plumbing that is not required to eliminate the current hang.
3. Do both timeout and cancellation now.
   Highest flexibility, but unnecessary scope for a simple bugfix.

### Task 1: Add the failing timeout test

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Test: `src-tauri/src/commands.rs`

**Step 1: Write the failing test**

Add a test for a small async helper that waits for rebase resolution and returns `Err("Timed out waiting for merge conflict resolution.")` when the rebase condition never clears within the configured poll budget.

**Step 2: Run test to verify it fails**

Run: `cargo test -p the-controller --lib commands::tests::test_wait_for_merge_rebase_resolution_times_out -- --nocapture`

Expected: FAIL because the helper does not exist yet or the timeout behavior is not implemented.

### Task 2: Implement the minimal merge timeout

**Files:**
- Modify: `src-tauri/src/commands.rs`

**Step 1: Write minimal implementation**

Add merge-specific timeout constants and a small async helper that polls `is_rebase_in_progress` up to the configured maximum before returning a timeout error. Use that helper from `merge_session_branch` in place of the infinite `loop`.

**Step 2: Run targeted test to verify it passes**

Run: `cargo test -p the-controller --lib commands::tests::test_wait_for_merge_rebase_resolution_times_out -- --nocapture`

Expected: PASS

### Task 3: Verify broader command behavior

**Files:**
- Modify: `src-tauri/src/commands.rs`

**Step 1: Run broader verification**

Run: `cargo test -p the-controller --lib commands::tests -- --nocapture`

Expected: PASS

**Step 2: Commit**

```bash
git add docs/plans/2026-03-10-merge-session-branch-timeout.md src-tauri/src/commands.rs
git commit -m "fix: timeout merge session branch conflict polling

closes #249

Contributed-by: auto-worker"
```
