# Scaffold Project Lock Scope Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Make `scaffold_project` release the global storage mutex before long-running scaffold work and run that work off the main thread.

**Architecture:** Extract the config and duplicate-name checks under a short-lived storage lock, perform the filesystem/git/GitHub scaffold flow inside `tokio::task::spawn_blocking`, then reacquire storage only for the final `save_project` call. Keep the existing rollback helpers and project construction logic so behavior changes only around lock scope and command scheduling.

**Tech Stack:** Rust, Tauri commands, `tokio::task::spawn_blocking`, `git2`, command tests in `src-tauri/src/commands.rs`

---

### Task 1: Add the lock-contention regression test

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Test: `src-tauri/src/commands.rs`

**Step 1: Write the failing test**

Add a command test that:
- Creates a test `AppState`.
- Uses fake `gh` and `git` binaries.
- Makes fake `gh repo create` wait until a sentinel file is removed.
- Starts `scaffold_project` on another thread.
- Waits for the fake `gh` script to signal that scaffolding reached the long-running external phase.
- Attempts `app_state.storage.try_lock()` and asserts it succeeds while scaffold is still blocked.

**Step 2: Run test to verify it fails**

Run: `cargo test 'commands::tests::test_scaffold_project_does_not_hold_storage_lock_during_external_publish' --manifest-path src-tauri/Cargo.toml -- --exact`

Expected: FAIL because the current implementation still holds the storage mutex for the full scaffold.

### Task 2: Narrow the lock scope and offload blocking work

**Files:**
- Modify: `src-tauri/src/commands.rs`

**Step 1: Write minimal implementation**

- Change `scaffold_project` to `pub async fn`.
- Under an initial short-lived storage lock, load config and reject duplicates.
- Move the existing scaffold body into `spawn_blocking`.
- Reacquire storage only for the final duplicate-name recheck and `save_project`.
- If the name was claimed while scaffolding was in flight, roll back the scaffolded repo/remote before returning the duplicate-name error.

**Step 2: Run the new regression to verify it passes**

Run: `cargo test 'commands::tests::test_scaffold_project_does_not_hold_storage_lock_during_external_publish' --manifest-path src-tauri/Cargo.toml -- --exact`

Expected: PASS.

### Task 3: Re-run the existing scaffold regressions

**Files:**
- Modify: `src-tauri/src/commands.rs`

**Step 1: Run rollback-focused scaffold tests**

Run: `cargo test test_scaffold_project_rolls_back_directory_when_github_creation_fails --manifest-path src-tauri/Cargo.toml -- --exact`

Run: `cargo test test_scaffold_project_rolls_back_remote_and_local_state_when_initial_push_fails --manifest-path src-tauri/Cargo.toml -- --exact`

Run: `cargo test 'commands::tests::test_scaffold_project_rolls_back_if_name_is_claimed_before_final_save' --manifest-path src-tauri/Cargo.toml -- --exact`

Expected: PASS.

**Step 2: Run broader command coverage**

Run: `cargo test commands::tests:: --manifest-path src-tauri/Cargo.toml`

Expected: PASS.

### Task 4: Review and ship

**Files:**
- Modify: `docs/plans/2026-03-10-scaffold-lock-scope-design.md`
- Modify: `docs/plans/2026-03-10-scaffold-lock-scope-implementation.md`
- Modify: `src-tauri/src/commands.rs`

**Step 1: Self-review**

Run: `git diff -- docs/plans/2026-03-10-scaffold-lock-scope-design.md docs/plans/2026-03-10-scaffold-lock-scope-implementation.md src-tauri/src/commands.rs`

**Step 2: Verify before commit**

Run the full chosen verification command and confirm fresh output before commit or PR creation.

**Step 3: Commit**

Use a commit message whose body includes `closes #307` and ends with:

`Contributed-by: auto-worker`
