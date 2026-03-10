# AppState Storage Init Failure Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Prevent startup panics when app storage cannot be initialized and replace them with graceful startup failure.

**Architecture:** Move storage initialization onto fallible constructors and handle the failure once in the Tauri bootstrap. Keep `AppState`'s stored fields unchanged so the rest of the app continues to use managed state the same way.

**Tech Stack:** Rust, Tauri v2, `rfd` native dialogs, Rust unit tests

---

### Task 1: Make storage and app-state initialization fallible

**Files:**
- Modify: `src-tauri/src/storage.rs`
- Modify: `src-tauri/src/state.rs`

**Step 1: Write the failing test**

Add tests that:
- assert `AppState::from_storage(...)` returns an error when the storage base path is a file
- assert the default-path resolver returns an error when no home directory is available

**Step 2: Run test to verify it fails**

Run: `cargo test state::tests::test_app_state_from_storage_returns_error_when_storage_dirs_cannot_be_created`
Expected: FAIL because `AppState::from_storage` does not exist yet

**Step 3: Write minimal implementation**

- Add a fallible default-path helper in `Storage`
- Add `AppState::from_storage`
- Make `AppState::new()` return `std::io::Result<Self>`

**Step 4: Run test to verify it passes**

Run: `cargo test state::tests::test_app_state_from_storage_returns_error_when_storage_dirs_cannot_be_created`
Expected: PASS

**Step 5: Commit**

```bash
git add docs/plans/2026-03-10-app-state-storage-init-design.md docs/plans/2026-03-10-app-state-storage-init.md src-tauri/src/storage.rs src-tauri/src/state.rs
git commit -m "fix: make app state initialization fallible"
```

### Task 2: Handle startup failures in the Tauri bootstrap

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`

**Step 1: Write the failing test**

No direct bootstrap unit test is needed beyond Task 1 because the correctness hinge is the constructor contract. Keep Task 1 as the regression coverage.

**Step 2: Implement minimal bootstrap handling**

- Add `rfd` as a dependency
- Attempt `state::AppState::new()` before `.manage(...)`
- On failure, show a native error dialog and return early

**Step 3: Run focused verification**

Run: `cargo test state::tests::test_app_state_from_storage_returns_error_when_storage_dirs_cannot_be_created`
Expected: PASS

**Step 4: Run broader verification**

Run: `cargo test`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/lib.rs
git commit -m "fix: show startup error for storage init failures"
```
