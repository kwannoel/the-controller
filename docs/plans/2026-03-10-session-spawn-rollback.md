# Session Metadata Rollback On PTY Spawn Failure Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Prevent `create_session` and `unarchive_session` from leaving persisted session metadata behind when PTY spawn fails, without clobbering concurrent project updates.

**Architecture:** Keep the existing save-before-spawn ordering, but route both command paths through a transactional helper in `commands.rs` that applies a compensating rollback mutation to the latest reloaded `Project` if the post-save action fails. For `create_session`, also clean up the worktree created for the failed session spawn. Exercise the rollback behavior with focused command tests that simulate spawn failure via an injected closure.

**Tech Stack:** Rust, Tauri command state/storage helpers, existing tests in `src-tauri/src/commands.rs`

---

### Task 1: Add the failing regression tests

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Test: `src-tauri/src/commands.rs`

**Step 1: Write the failing tests**

Add two tests around a new transactional helper:
- One mutates a project by appending a new `SessionConfig`, then forces the post-save action to fail and asserts the saved project still has zero sessions.
- One starts with an archived session, mutates it to unarchived, then forces the post-save action to fail and asserts the saved project still shows the session as archived.

**Step 2: Run tests to verify they fail**

Run: `cargo test rollback_session_metadata --manifest-path src-tauri/Cargo.toml -- --nocapture`

Expected: FAIL because the saved project still reflects the post-save mutation after the injected failure.

### Task 2: Implement the minimal rollback helper

**Files:**
- Modify: `src-tauri/src/commands.rs`

**Step 1: Write minimal implementation**

- Add a helper that loads a project, applies a mutation, saves, runs a post-save closure, and on failure reloads the latest project state and applies a compensating rollback mutation.
- Preserve concurrent unrelated project changes by rolling back only the affected session mutation.
- Preserve the original action error unless rollback also fails.

**Step 2: Run targeted tests to verify they pass**

Run: `cargo test rollback_session_metadata --manifest-path src-tauri/Cargo.toml -- --nocapture`

Expected: PASS.

### Task 3: Wire the commands to the helper

**Files:**
- Modify: `src-tauri/src/commands.rs`

**Step 1: Replace duplicated save-before-spawn logic**

- Update `create_session` to persist the new `SessionConfig` via the helper and run `spawn_session` as the post-save action.
- Update `unarchive_session` to toggle `archived` via the helper and run `spawn_session` as the post-save action.
- Remove the newly created worktree/branch when `create_session` spawn fails.
- Keep lock ordering safe by ensuring storage is not held while the PTY manager lock is acquired.

**Step 2: Run focused command tests**

Run: `cargo test commands::tests:: --manifest-path src-tauri/Cargo.toml rollback_session_metadata`

Expected: PASS.

### Task 4: Cover the concurrency and cleanup edge cases

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Test: `src-tauri/src/commands.rs`

**Step 1: Add regression coverage**

- Add a test proving unrelated concurrent project edits survive the rollback path.
- Add a focused test proving failed create-session cleanup removes the created worktree and branch reference.

**Step 2: Run focused command tests**

Run: `cargo test commands::tests:: --manifest-path src-tauri/Cargo.toml`

Expected: PASS.

### Task 5: Verify, review, and ship

**Files:**
- Modify: `docs/plans/2026-03-10-session-spawn-rollback-design.md`
- Modify: `docs/plans/2026-03-10-session-spawn-rollback.md`

**Step 1: Run fresh verification**

Run the targeted regression tests plus the relevant command suite and confirm the output is passing before commit/PR.

**Step 2: Self-review diff**

Run: `git diff -- src-tauri/src/commands.rs docs/plans/2026-03-10-session-spawn-rollback-design.md docs/plans/2026-03-10-session-spawn-rollback.md`

**Step 3: Commit**

Use a commit message body that includes `closes #298` and ends with:

`Contributed-by: auto-worker`
