# Auto-Worker Label Standardization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Standardize issue labels across maintainer, triage, manual issue creation, and auto-worker so the worker can reliably pick up tasks, then migrate existing GitHub issues to the standardized taxonomy.

**Architecture:** Keep one canonical label taxonomy: `priority:high|low` and `complexity:simple|high`. Update all frontend label emitters to write the canonical labels. In the auto-worker scheduler, migrate existing GitHub issues from legacy labels to the canonical set before issue selection, then select only canonical labels.

**Tech Stack:** Rust (`gh` CLI, Tauri backend), Svelte 5 frontend, Rust unit tests, Vitest where needed.

---

### Task 1: Lock the behavior with failing Rust tests

**Files:**
- Modify: `src-tauri/src/auto_worker.rs`

**Step 1: Write the failing test**

Add tests for:
- standardized labels `priority:high` + `complexity:simple` are eligible
- legacy labels are mapped to canonical labels during migration
- legacy labels are removed without adding unrelated labels during migration

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test auto_worker::tests::`

Expected: at least the new eligibility test fails before implementation.

**Step 3: Write minimal implementation**

Add pure helper functions in `auto_worker.rs` that:
- determine eligibility from canonical labels only
- compute add/remove label actions for migrating one issue

**Step 4: Run test to verify it passes**

Run the same `cargo test` command and verify green.

### Task 2: Migrate live issues before worker selection

**Files:**
- Modify: `src-tauri/src/auto_worker.rs`

**Step 1: Write the failing test**

Add tests for migration action generation:
- `priority: high` -> `priority:high`
- `priority: low` -> `priority:low`
- `complexity: low` -> `complexity:simple`
- `complexity: high` -> `complexity:high`
- migration does not add unrelated labels such as `triaged`

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test auto_worker::tests::migration_plan_rewrites_legacy_labels`

Expected: fail until action computation exists.

**Step 3: Write minimal implementation**

In the scheduler path:
- ensure canonical labels exist
- fetch issues
- migrate legacy labels on GitHub with `gh issue edit`
- update the in-memory issue list before calling `pick_eligible_issue`

**Step 4: Run test to verify it passes**

Run the focused `cargo test` command again.

### Task 3: Update all label emitters to canonical labels

**Files:**
- Modify: `src/App.svelte`
- Modify: `src/lib/TriagePanel.svelte`
- Modify: `src/lib/IssuePickerModal.svelte`
- Modify: `src/lib/AgentDashboard.svelte`

**Step 1: Write the failing test**

Prefer updating existing UI tests only if they already cover label text or sorting behavior. Otherwise keep this task implementation-only and verify via deterministic code review plus backend tests.

**Step 2: Write minimal implementation**

Update:
- manual issue creation labels in `App.svelte`
- triage labels in `TriagePanel.svelte`
- issue priority grouping in `IssuePickerModal.svelte`
- policy text in `AgentDashboard.svelte`

**Step 3: Run targeted verification**

Run: `npm test -- --runInBand` only if a directly affected frontend test exists and stays cheap; otherwise rely on `npx vitest run src/lib/commands.test.ts src/App.test.ts` only if relevant.

### Task 4: Validate with live GitHub migration and worker readiness

**Files:**
- No code changes expected

**Step 1: Run backend verification**

Run: `cd src-tauri && cargo test auto_worker`

**Step 2: Run live migration trigger**

Run the app or a targeted backend path that executes the auto-worker scheduler against the enabled `the-controller` project so existing issues are migrated.

**Step 3: Validate agent behavior**

Verify on the live repo:
- target issues now carry canonical labels
- at least one issue has `priority:high` and `complexity:simple`
- the auto-worker has an eligible issue to spawn against

**Step 4: Record residual risks**

If full end-to-end session spawning cannot be exercised in this turn, explicitly report that gap and the evidence gathered.
