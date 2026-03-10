# Auto-Worker Assignment Label Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Make auto-worker issue provenance explicit with `assigned-to-auto-worker` and derive completed worker history from closed issues instead of the fragile `finished-by-worker` label.

**Architecture:** Update the auto-worker scheduler so issue claim and cleanup maintain two labels with distinct semantics: `in-progress` for transient ownership and `assigned-to-auto-worker` for persistent worker provenance. Update the GitHub command that feeds the dashboard to query closed worker-labeled issues, and run a one-off migration to backfill historical worker-owned issues from existing worker report comments.

**Tech Stack:** Rust, Tauri v2, TypeScript, Svelte 5, Vitest, GitHub CLI

---

### Task 1: Add failing backend coverage for worker label semantics

**Files:**
- Modify: `src-tauri/src/auto_worker.rs`
- Test: `src-tauri/src/auto_worker.rs`

**Step 1: Write the failing test**

Add focused unit tests for helper logic that encodes:
- claiming an issue adds `assigned-to-auto-worker`
- successful completion keeps `assigned-to-auto-worker`
- unsuccessful completion removes `assigned-to-auto-worker`

Factor pure label-transition helpers if needed so the logic is testable without shelling out to GitHub.

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test auto_worker -- --nocapture`
Expected: FAIL because current worker cleanup logic has no `assigned-to-auto-worker` behavior.

### Task 2: Implement minimal backend label transition logic

**Files:**
- Modify: `src-tauri/src/auto_worker.rs`
- Test: `src-tauri/src/auto_worker.rs`

**Step 1: Write minimal implementation**

Update the scheduler to:
- define `LABEL_ASSIGNED_TO_AUTO_WORKER`
- add it alongside `in-progress` when a worker claims an issue
- remove `in-progress` on all worker cleanup paths
- keep or remove `assigned-to-auto-worker` based on whether the issue is closed
- stop using `finished-by-worker` for eligibility and completion bookkeeping where no longer needed

Use the smallest refactor that makes cleanup decisions explicit and shared across normal exit, kill, and restart recovery.

**Step 2: Run targeted tests to verify they pass**

Run: `cd src-tauri && cargo test auto_worker -- --nocapture`
Expected: PASS

### Task 3: Add failing coverage for completed worker issue queries

**Files:**
- Modify: `src-tauri/src/commands/github.rs`
- Test: `src-tauri/src/commands/github.rs`

**Step 1: Write the failing test**

Extract a pure parser/helper for worker issue query results and add tests asserting:
- closed issues with `assigned-to-auto-worker` are returned
- open issues with `assigned-to-auto-worker` are excluded from completed history
- worker report comments are still selected when present

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test commands::github -- --nocapture`
Expected: FAIL because the current query/parser is keyed to `finished-by-worker`.

### Task 4: Implement the new completed-worker query

**Files:**
- Modify: `src-tauri/src/commands/github.rs`
- Modify: `src-tauri/src/commands.rs`
- Test: `src-tauri/src/commands/github.rs`

**Step 1: Write minimal implementation**

Change `get_worker_reports` to:
- query issues labeled `assigned-to-auto-worker`
- request issue state along with comments and timestamps
- filter to closed issues before returning results
- keep returning the latest `<!-- auto-worker-report -->` comment body when present
- return a fallback body for closed worker issues with no report comment so they still show in the auto-worker pane

Preserve the existing `WorkerReport` API shape unless the frontend needs a small extension.

**Step 2: Run targeted tests to verify they pass**

Run: `cd src-tauri && cargo test commands::github -- --nocapture`
Expected: PASS

### Task 5: Update the dashboard wording and behavior

**Files:**
- Modify: `src/lib/AgentDashboard.svelte`
- Modify: `src/lib/stores.ts`
- Test: `src/lib/AgentDashboard.test.ts` or the closest existing dashboard/frontend test file if present

**Step 1: Write the failing test**

Add or extend frontend coverage so the auto-worker dashboard treats the returned items as completed work and does not depend on `finished-by-worker` terminology.

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/AgentDashboard.test.ts`
Expected: FAIL because the current wording/query assumptions still reflect the old finished-label model.

**Step 3: Write minimal implementation**

Update any user-facing copy that still implies the old terminal label model if needed, while keeping the dashboard interaction model unchanged. Ensure the auto-worker pane still renders the completed issue list and detail view even when some items use fallback report text.

**Step 4: Run targeted tests to verify they pass**

Run: `npx vitest run src/lib/AgentDashboard.test.ts`
Expected: PASS

### Task 6: Add a one-off migration for historical worker issues

**Files:**
- Create: `scripts/backfill-auto-worker-assignment-labels.sh`
- Modify: `docs/plans/2026-03-10-auto-worker-assignment-label-design.md`
- Modify: `docs/plans/2026-03-10-auto-worker-assignment-label.md`

**Step 1: Write the migration script**

Create a small script that:
- lists repo issues with comments
- finds issues containing `<!-- auto-worker-report -->`
- adds `assigned-to-auto-worker` to those issues
- optionally removes `finished-by-worker` if present

Keep it idempotent and repository-parameterized.

**Step 2: Dry-run or inspect the candidate set**

Run: `bash scripts/backfill-auto-worker-assignment-labels.sh kwannoel/the-controller --dry-run`
Expected: Prints the historical worker-owned issues that will be labeled.

**Step 3: Run the migration**

Run: `bash scripts/backfill-auto-worker-assignment-labels.sh kwannoel/the-controller`
Expected: Historical issues receive `assigned-to-auto-worker` without changing unrelated issues.

### Task 7: Full verification and review

**Files:**
- Modify: `src-tauri/src/auto_worker.rs`
- Modify: `src-tauri/src/commands/github.rs`
- Modify: `src/lib/AgentDashboard.svelte`
- Create: `scripts/backfill-auto-worker-assignment-labels.sh`

**Step 1: Run backend verification**

Run: `cd src-tauri && cargo test`
Expected: PASS

**Step 2: Run frontend verification**

Run: `npx vitest run`
Expected: PASS

**Step 3: Review the GitHub issue state**

Verify in GitHub that:
- active worker issues carry both `in-progress` and `assigned-to-auto-worker`
- completed worker issues are closed and retain `assigned-to-auto-worker`
- abandoned or failed worker issues are open and do not retain `assigned-to-auto-worker`
- completed worker issues remain visible in the auto-worker pane, with real report comments when available and fallback text otherwise

**Step 4: Commit**

```bash
git add src-tauri/src/auto_worker.rs src-tauri/src/commands/github.rs src-tauri/src/commands.rs src/lib/AgentDashboard.svelte src/lib/stores.ts scripts/backfill-auto-worker-assignment-labels.sh docs/plans/2026-03-10-auto-worker-assignment-label-design.md docs/plans/2026-03-10-auto-worker-assignment-label.md
git commit -m "fix: add persistent auto-worker assignment label"
```
