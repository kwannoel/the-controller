# Project JSON Corruption Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Surface corrupt `project.json` entries without dropping valid projects, and block duplicate-sensitive operations when metadata corruption exists.

**Architecture:** Replace the raw project-list return shape with a project-scan result that carries both valid projects and corruption diagnostics. Commands and UI consume that richer shape, while duplicate-sensitive backend operations explicitly fail when corruption is present.

**Tech Stack:** Rust, Tauri v2 commands, Svelte 5, Vitest, cargo test

---

### Task 1: Add failing storage regression coverage

**Files:**

- Modify: `src-tauri/src/storage.rs`
- Test: `src-tauri/src/storage.rs`

**Step 1: Write the failing test**

Add a storage test that writes one valid `project.json` and one malformed `project.json`, then asserts the scan result contains the valid project and a corruption entry for the malformed file.

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test storage::tests::test_scan_projects_reports_corrupt_entries -- --exact`

Expected: FAIL because the scan API does not exist yet.

**Step 3: Write minimal implementation**

Add scan result types and storage scanning logic that records corrupt metadata entries instead of silently dropping them.

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test storage::tests::test_scan_projects_reports_corrupt_entries -- --exact`

Expected: PASS

**Step 5: Commit**

Commit the storage regression and implementation once green.

### Task 2: Add failing command/UI regression coverage

**Files:**

- Modify: `src-tauri/src/commands.rs`
- Modify: `src/lib/stores.ts`
- Modify: `src/lib/Sidebar.svelte`
- Test: `src-tauri/src/commands.rs`
- Test: `src/lib/Sidebar.test.ts`

**Step 1: Write the failing tests**

Add:

- a Rust command test asserting duplicate-sensitive project creation/load returns an explicit corruption error when scan diagnostics exist
- a Sidebar test asserting corruption warnings render when `list_projects` returns diagnostics

**Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test commands::tests::test_create_project_rejects_corrupt_project_metadata -- --exact`

Run: `npx vitest run src/lib/Sidebar.test.ts`

Expected: FAIL because the command payload and UI do not handle corruption diagnostics yet.

**Step 3: Write minimal implementation**

Update command payloads, frontend types/state, and sidebar rendering to surface corruption warnings. Update duplicate-sensitive command paths to return a clear corruption error.

**Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test commands::tests::test_create_project_rejects_corrupt_project_metadata -- --exact`

Run: `npx vitest run src/lib/Sidebar.test.ts`

Expected: PASS

**Step 5: Commit**

Commit once Rust and frontend regressions are green.

### Task 3: Update background callers and run full verification

**Files:**

- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/maintainer.rs`
- Modify: `src-tauri/src/auto_worker.rs`
- Modify: `src-tauri/src/status_socket.rs`

**Step 1: Update callers**

Switch restore, scheduler, and cleanup paths to the project-scan API so they log explicit corruption diagnostics while still acting on valid projects.

**Step 2: Run focused verification**

Run: `cd src-tauri && cargo test`

Run: `npx vitest run`

Expected: PASS

**Step 3: Run final repo verification**

Run any broader project verification needed before commit/PR, confirm fresh output, then proceed to review and merge workflow.

**Step 4: Commit**

Commit the finished change with issue reference and worker trailer.
