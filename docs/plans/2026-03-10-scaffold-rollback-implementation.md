# Scaffold Rollback Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Make `scaffold_project` clean up partial local and remote scaffold state when GitHub publishing fails so the user can retry immediately.

**Architecture:** Keep scaffolding local setup the same, but split GitHub publishing into `gh repo create --remote=origin` and `git push --set-upstream origin HEAD`. Roll back the local directory when repo creation fails, and roll back both the created remote and local directory when the initial push fails. Keep project persistence after publish success so there is no partial storage state to clean up.

**Tech Stack:** Rust, git2, std::fs, existing command tests in `src-tauri/src/commands.rs`

---

### Task 1: Document the failing behavior with a regression test

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Test: `src-tauri/src/commands.rs`

**Step 1: Write the failing test**

Add a command-level test that:
- Creates a temp projects root.
- Uses fake `gh` and `git` binaries to force `gh repo create` to fail.
- Calls `scaffold_project`.
- Asserts the repo directory no longer exists.
- Asserts no project metadata was saved.
- Retries with successful fake binaries and asserts the directory exists.

**Step 2: Run test to verify it fails**

Run: `cargo test test_scaffold_project_rolls_back_directory_when_github_creation_fails --manifest-path src-tauri/Cargo.toml -- --exact`

Expected: FAIL because the repo directory is still present after the simulated GitHub failure.

### Task 2: Implement rollback for both publish failure points

**Files:**
- Modify: `src-tauri/src/commands.rs`

**Step 1: Write minimal implementation**

- Add helper functions for `gh` and `git` command construction plus rollback of local and remote scaffold state.
- Change `scaffold_project` to call `gh repo create --remote=origin` first and `git push --set-upstream origin HEAD` second.
- On `gh repo create` failure, remove the local repo directory.
- On push failure, delete the created remote and then remove the local repo directory.

**Step 2: Run targeted test to verify it passes**

Run: `cargo test test_scaffold_project_rolls_back_directory_when_github_creation_fails --manifest-path src-tauri/Cargo.toml -- --exact`

Expected: PASS.

### Task 3: Cover the push-failure edge case and surrounding behavior

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Test: `src-tauri/src/commands.rs`

**Step 1: Add push-failure regression**

Add a second command-level test that:
- Uses fake `gh` to create an `origin` remote successfully.
- Uses fake `git` to fail the first push and succeed on retry.
- Verifies both local directory cleanup and remote deletion on failure.
- Verifies retry recreates the remote and saves the project.

**Step 2: Run relevant command and integration tests**

Run: `cargo test commands::tests:: --manifest-path src-tauri/Cargo.toml`

Run: `cargo test test_scaffold --manifest-path src-tauri/Cargo.toml`

Expected: PASS.

### Task 4: Review and ship

**Files:**
- Modify: `docs/plans/2026-03-10-scaffold-rollback-design.md`
- Modify: `docs/plans/2026-03-10-scaffold-rollback-implementation.md`

**Step 1: Self-review diff**

Run: `git diff -- src-tauri/src/commands.rs src-tauri/tests/integration.rs docs/plans/2026-03-10-scaffold-rollback-design.md docs/plans/2026-03-10-scaffold-rollback-implementation.md`

**Step 2: Verify before commit**

Run the full verification command chosen for this fix and confirm fresh passing output before commit/PR.

**Step 3: Commit**

Use a commit message that includes `closes #328` in the body and ends with:

`Contributed-by: auto-worker`
