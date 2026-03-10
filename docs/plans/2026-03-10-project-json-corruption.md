# Surface Corrupt Project JSON Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Preserve valid projects while explicitly surfacing corrupt `project.json` entries in backend results and the UI.

**Architecture:** Replace the raw `Vec<Project>` list result with a structured inventory that carries both valid projects and corruption diagnostics. Update the sidebar list-loading path to consume the richer result and show a warning toast for newly detected corrupt entries.

**Tech Stack:** Rust, Tauri v2 commands, Svelte 5, Vitest

---

### Task 1: Add Storage Regression Coverage

**Files:**

- Modify: `src-tauri/src/storage.rs`
- Test: `src-tauri/src/storage.rs`

**Step 1: Write the failing test**

Add a storage test that:

- saves one valid project
- writes an invalid `project.json` in a second project directory
- calls `storage.list_projects()`
- asserts the valid project remains in `projects`
- asserts `corrupt_entries.len() == 1`
- asserts the corrupt entry includes the bad `project.json` path

**Step 2: Run test to verify it fails**

Run: `cargo test test_list_projects_reports_corrupt_project_json`
Expected: FAIL because `list_projects` still returns `Vec<Project>` and cannot report corrupt entries.

**Step 3: Write minimal implementation**

Introduce the inventory/result structs in Rust and update `Storage::list_projects` to collect corrupt entries instead of silently dropping them.

**Step 4: Run test to verify it passes**

Run: `cargo test test_list_projects_reports_corrupt_project_json`
Expected: PASS

**Step 5: Commit**

Commit after frontend work as part of the full issue fix.

### Task 2: Add Sidebar Warning Coverage

**Files:**

- Modify: `src/lib/Sidebar.test.ts`
- Modify: `src/lib/stores.ts`
- Modify: `src/lib/Sidebar.svelte`

**Step 1: Write the failing test**

Add a `Sidebar` test that mocks `invoke("list_projects")` to return:

- one valid project
- one corrupt entry

Assert:

- the valid projects store is updated
- `showToast` is called with an error warning mentioning corrupt `project.json`

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/Sidebar.test.ts`
Expected: FAIL because `Sidebar` currently expects `Project[]` and never surfaces corruption warnings.

**Step 3: Write minimal implementation**

Add frontend list-result types, update `loadProjects`/`loadArchivedProjects`, and warn once per unique corrupt entry.

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/Sidebar.test.ts`
Expected: PASS

**Step 5: Commit**

Commit after backend and frontend changes are verified together.

### Task 3: Update Command Consumers And Verify End-To-End

**Files:**

- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/auto_worker.rs`
- Modify: `src-tauri/src/maintainer.rs`
- Modify: `src-tauri/src/status_socket.rs`
- Modify: any other Rust caller of `storage.list_projects()`

**Step 1: Write the failing test**

Use the earlier storage regression as the invariant-proving test; reverting the storage/command changes should reintroduce the failure.

**Step 2: Run targeted verification**

Run:

- `cd src-tauri && cargo test test_list_projects_reports_corrupt_project_json`
- `npx vitest run src/lib/Sidebar.test.ts`

Expected: PASS after implementation.

**Step 3: Run broader verification**

Run:

- `cd src-tauri && cargo test`
- `npx vitest run`

Expected: PASS

**Step 4: Self-review**

Check the diff for:

- unchanged behavior with all-valid projects
- no repeated toast spam
- stderr warnings for background callers that cannot show UI

**Step 5: Commit**

Use a commit message that includes `closes #327` and the required trailer:

```text
fix: surface corrupt project metadata

closes #327

Contributed-by: auto-worker
```
