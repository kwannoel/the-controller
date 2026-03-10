# Auto-Worker Eligible Issues Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Show a navigable eligible-issues queue in the auto-worker panel, including the currently active issue, while preserving completed worker reports in a separate panel view.

**Architecture:** Add a backend queue command that reuses `auto_worker::is_eligible` and augments the result with the current active issue. Extend `AgentDashboard.svelte` with an auto-worker `Queue`/`Reports` view model, queue list/detail state, and keyboard/browser-open support parallel to the existing maintainer issue flow.

**Tech Stack:** Rust, Tauri v2 commands, Svelte 5 runes, TypeScript, Vitest

**Design doc:** `docs/plans/2026-03-10-auto-worker-eligible-issues-design.md`

---

### Task 1: Add the failing frontend queue test

**Files:**
- Modify: `src/lib/AgentDashboard.test.ts`

**Step 1: Write the failing test**

Add a test that renders the auto-worker panel, mocks a queue response plus reports, verifies the queue view is shown by default, opens an eligible issue, and confirms the active issue is marked as working.

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/AgentDashboard.test.ts`
Expected: FAIL because the auto-worker panel does not fetch or render a queue yet.

**Step 3: Commit**

```bash
git add src/lib/AgentDashboard.test.ts
git commit -m "test: cover auto-worker eligible queue panel"
```

---

### Task 2: Add backend queue types and command

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/commands/github.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/auto_worker.rs`
- Test: `src-tauri/src/commands.rs`

**Step 1: Add a serializable queue item model**

Add a model for the auto-worker queue response with `number`, `title`, `url`, `body`, `labels`, and `is_active`.

**Step 2: Add queue-building logic**

Implement a helper that:
- lists issues for the repo
- filters them through `auto_worker::is_eligible`
- prepends the active issue when the scheduler reports `working`
- avoids duplicates when the active issue also appears in the eligible set

**Step 3: Expose a Tauri command**

Add `get_auto_worker_queue(project_id)` that loads the project, derives repo info, and returns queue items.

**Step 4: Add backend tests**

Cover:
- eligible issues are returned
- ineligible issues are excluded
- active issue is included even if it is no longer eligible
- active issue is not duplicated

**Step 5: Run backend tests**

Run: `cd src-tauri && cargo test auto_worker`
Expected: PASS with the new queue behavior covered.

**Step 6: Commit**

```bash
git add src-tauri/src/models.rs src-tauri/src/commands/github.rs src-tauri/src/commands.rs src-tauri/src/lib.rs src-tauri/src/auto_worker.rs
git commit -m "feat: expose auto-worker eligible queue"
```

---

### Task 3: Implement the auto-worker queue UI

**Files:**
- Modify: `src/lib/stores.ts`
- Modify: `src/lib/AgentDashboard.svelte`

**Step 1: Extend frontend queue types**

Add a TypeScript interface for the auto-worker queue item response.

**Step 2: Add failing detail-state assertions if needed**

If the first test is too broad, split out a second failing assertion for queue detail opening or browser-open behavior.

**Step 3: Implement minimal queue fetch and state**

Add:
- auto-worker view mode state (`queue` / `reports`)
- queue list loading state
- queue detail state
- fetch logic for `get_auto_worker_queue`
- refresh on focus and status changes

**Step 4: Render the queue view**

Show:
- working badge for the active issue
- queued issue rows
- `No eligible issues` empty state
- detail pane for the selected issue

**Step 5: Wire keyboard and browser-open behavior**

Reuse existing agent-panel navigation for queue list/detail and route open-in-browser to the selected queue item when relevant.

**Step 6: Run frontend tests**

Run: `npx vitest run src/lib/AgentDashboard.test.ts`
Expected: PASS with queue rendering and navigation covered.

**Step 7: Commit**

```bash
git add src/lib/stores.ts src/lib/AgentDashboard.svelte src/lib/AgentDashboard.test.ts
git commit -m "feat: show auto-worker eligible issues in panel"
```

---

### Task 4: Verify the integrated behavior

**Files:**
- Modify: none unless verification finds gaps

**Step 1: Run targeted frontend and backend verification**

Run: `npx vitest run src/lib/AgentDashboard.test.ts`
Expected: PASS

Run: `cd src-tauri && cargo test get_auto_worker_queue`
Expected: PASS

**Step 2: Run broader regression coverage**

Run: `npx vitest run`
Expected: PASS or only pre-existing unrelated failures

**Step 3: Document any residual risk**

If a full end-to-end agent validation is not run, note that the queue refresh timing still depends on status events and GitHub CLI availability.
