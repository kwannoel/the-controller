# Unreadable Enumeration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan task-by-task.

**Goal:** Keep project and maintainer-log enumeration working when one scanned file is unreadable.

**Architecture:** Localize the behavior change in `Storage` so callers keep their existing APIs. Project scans should convert per-file read failures into corrupt inventory entries, and maintainer log scans should skip unreadable files after logging a warning.

**Tech Stack:** Rust, Tauri v2, Cargo tests

---

### Task 1: Add Failing Storage Regressions

**Files:**
- Modify: `src-tauri/src/storage.rs`
- Test: `src-tauri/src/storage.rs`

**Step 1: Write the failing tests**

Add two tests:

- `test_list_projects_reports_unreadable_project_json_as_corrupt`
- `test_run_log_history_skips_unreadable_log_files`

Both tests should combine one valid JSON file with one unreadable JSON file in the same directory being scanned.

**Step 2: Run tests to verify they fail**

Run:

- `cargo test --manifest-path src-tauri/Cargo.toml storage::tests::test_list_projects_reports_unreadable_project_json_as_corrupt`
- `cargo test --manifest-path src-tauri/Cargo.toml storage::tests::test_run_log_history_skips_unreadable_log_files`

Expected: FAIL because the current storage code does not record unreadable project files as corrupt and still aborts maintainer log history on unreadable files.

**Step 3: Write minimal implementation**

- In `list_projects`, push a `CorruptProjectEntry` when `read_to_string` fails.
- In `load_run_logs_from_dir`, warn and continue when `read_to_string` fails.

**Step 4: Run tests to verify they pass**

Run the same two commands and confirm both pass.

**Step 5: Commit**

Use a conventional commit with:

```text
fix: tolerate unreadable enumerated files

closes #16

Contributed-by: auto-worker
```

### Task 2: Verify The Repo Gates

**Files:**
- Modify: `src-tauri/src/storage.rs`

**Step 1: Run targeted storage coverage**

Run:

- `cargo test --manifest-path src-tauri/Cargo.toml storage::tests`

**Step 2: Run required repo verification**

Run:

- `pnpm check`
- `cargo fmt --check --manifest-path src-tauri/Cargo.toml`
- `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`

**Step 3: Self-review**

Inspect the diff for:

- no API churn outside storage
- warning-only handling for unreadable maintainer logs
- preserved corruption reporting for project inventory

**Step 4: Commit if clean**

Stage the docs and code, then create the commit from Task 1.
