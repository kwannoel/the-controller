# Auto-worker Label Migration Cleanup Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Remove legacy auto-worker label migration and support only canonical GitHub labels.

**Architecture:** Keep the scheduler simple: it fetches issues and filters only on canonical labels. No startup relabeling, no compatibility rewriting, and no tests that imply legacy labels still work.

**Tech Stack:** Rust, cargo test

---

### Task 1: Lock in strict canonical behavior with tests

**Files:**
- Modify: `src-tauri/src/auto_worker.rs`

**Step 1: Write the failing test**

Add a test asserting an issue with legacy labels like `priority: high` and `complexity: low` is not eligible.

**Step 2: Run test to verify it fails**

Run: `cargo test legacy_labels_are_not_eligible --manifest-path src-tauri/Cargo.toml`
Expected: FAIL because migration compatibility still exists in the file.

**Step 3: Remove migration-focused tests**

Delete tests that validate migration plans or migrated labels, since that behavior is being removed.

**Step 4: Run focused test suite**

Run: `cargo test auto_worker::tests --manifest-path src-tauri/Cargo.toml`
Expected: FAIL until production migration code is removed.

### Task 2: Remove legacy migration code from auto_worker

**Files:**
- Modify: `src-tauri/src/auto_worker.rs`

**Step 1: Remove startup migration execution**

Delete the background migration thread from `AutoWorkerScheduler::start`.

**Step 2: Remove migration helpers and constants**

Delete legacy-label constants, `LabelMigration`, migration helpers, and any now-unused label bootstrap that only existed for migration.

**Step 3: Keep canonical eligibility only**

Leave `is_eligible` using only canonical labels.

**Step 4: Run tests**

Run: `cargo test auto_worker::tests --manifest-path src-tauri/Cargo.toml`
Expected: PASS.

### Task 3: Verify cleanup scope

**Files:**
- Modify: `src-tauri/src/auto_worker.rs`

**Step 1: Search for removed concepts**

Run: `rg 'migrate_labels_background|migrate_issues_sync|LabelMigration|priority: high|complexity: low|complexity:simple' src-tauri/src/auto_worker.rs`
Expected: no matches.

**Step 2: Final verification**

Run: `cargo test auto_worker::tests --manifest-path src-tauri/Cargo.toml`
Expected: PASS.
