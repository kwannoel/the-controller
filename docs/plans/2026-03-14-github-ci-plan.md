# GitHub CI Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Update GitHub Actions CI to match current repo standards and split checks into clearer, parallel jobs.

**Architecture:** Keep a single workflow file and improve it rather than introducing more workflows. Add a stronger regression test around the workflow contract, then update the workflow to use `main`, `pnpm`, and separate frontend, Rust lint, and Rust test jobs with shared dependency setup patterns.

**Tech Stack:** GitHub Actions YAML, pnpm, Node 20, Rust stable, Vitest

---

### Task 1: Expand the CI workflow regression test

**Files:**
- Modify: `src/lib/ci-workflow.test.ts`
- Test: `src/lib/ci-workflow.test.ts`

**Step 1: Write the failing test**

Add assertions for:
- `branches: [main]`
- `cache: 'pnpm'`
- `pnpm/action-setup`
- `pnpm check`
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test`

**Step 2: Run test to verify it fails**

Run: `pnpm test -- src/lib/ci-workflow.test.ts`
Expected: FAIL because the current workflow still references `master`, `npm`, and misses some required commands.

**Step 3: Write minimal implementation**

Update `.github/workflows/ci.yml` so the assertions become true while keeping Linux system dependencies in place.

**Step 4: Run test to verify it passes**

Run: `pnpm test -- src/lib/ci-workflow.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add src/lib/ci-workflow.test.ts .github/workflows/ci.yml
git commit -m "ci: align GitHub Actions with repo checks"
```

### Task 2: Verify repository gates still pass

**Files:**
- Verify only: `.github/workflows/ci.yml`

**Step 1: Run frontend gate**

Run: `pnpm check`
Expected: PASS

**Step 2: Run Rust formatting gate**

Run: `cd src-tauri && cargo fmt --check`
Expected: PASS

**Step 3: Run Rust lint gate**

Run: `cd src-tauri && cargo clippy -- -D warnings`
Expected: PASS

**Step 4: Run Rust tests**

Run: `cd src-tauri && cargo test`
Expected: PASS if the local environment matches CI requirements

**Step 5: Commit**

```bash
git add .github/workflows/ci.yml src/lib/ci-workflow.test.ts docs/plans/2026-03-14-github-ci-design.md docs/plans/2026-03-14-github-ci-plan.md
git commit -m "docs: add GitHub CI design and plan"
```
