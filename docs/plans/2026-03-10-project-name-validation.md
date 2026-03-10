# Project Name Validation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Ensure `create_project` and `load_project` reject invalid project names using the existing shared validator.

**Architecture:** Keep the shared validation helper as the single source of truth and call it from the two missing Tauri command entry points. Prove the behavior with direct command tests so reverting either call reintroduces a failing regression.

**Tech Stack:** Rust, Tauri v2 commands, cargo test

---

### Task 1: Add Regression Tests For Missing Validation

**Files:**

- Modify: `src-tauri/src/commands.rs`
- Test: `src-tauri/src/commands.rs`

**Step 1: Write the failing test**

Add one test that calls `create_project` with an invalid project name and asserts it returns an error containing `Invalid project name`.

Add one test that calls `load_project` with an invalid project name against a real temporary git repo and asserts it returns the same error.

**Step 2: Run test to verify it fails**

Run:

- `cargo test test_create_project_rejects_invalid_project_name`
- `cargo test test_load_project_rejects_invalid_project_name`

Expected: both FAIL because the commands currently save invalid names instead of validating them.

**Step 3: Write minimal implementation**

Call `validate_project_name(&name)?;` at the top of `create_project` and `load_project`.

**Step 4: Run test to verify it passes**

Run the same two `cargo test` commands.

Expected: both PASS.

**Step 5: Commit**

Commit after targeted and broader verification are complete.

### Task 2: Verify And Review

**Files:**

- Modify: `src-tauri/src/commands.rs`
- Modify: `docs/plans/2026-03-10-project-name-validation-design.md`
- Modify: `docs/plans/2026-03-10-project-name-validation.md`

**Step 1: Run broader verification**

Run:

- `cd src-tauri && cargo test`

Expected: PASS.

**Step 2: Self-review**

Check the diff for:

- validation happening before persistence
- unchanged duplicate-name logic
- unchanged repo-path and git-repo checks
- tests that fail again if either validation call is removed

**Step 3: Commit**

Use a commit message body that includes the required trailer:

```text
fix: validate project names on create/load

closes #250

Contributed-by: auto-worker
```
