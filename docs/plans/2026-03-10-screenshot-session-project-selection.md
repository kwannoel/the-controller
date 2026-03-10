# Screenshot Session Project Selection Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Make screenshot-to-session create the new session in the currently focused project instead of a hardcoded project named `"the-controller"`.

**Architecture:** Reuse `App.svelte`'s existing focus-to-project helper so screenshot session targeting follows the same current-project model as the rest of the UI. Protect the behavior with an app-level regression test that proves a non-default project name still receives the screenshot session.

**Tech Stack:** Svelte 5, Vitest, Testing Library

---

### Task 1: Add Regression Coverage For Screenshot Project Selection

**Files:**

- Modify: `src/App.test.ts`
- Test: `src/App.test.ts`

**Step 1: Write the failing test**

Add a test that renders `App`, seeds `projects` with a single project whose name is not `"the-controller"`, focuses that project, triggers `hotkeyAction` with `type: "screenshot-to-session"`, and asserts `invoke("create_session", ...)` receives that focused project's id.

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/App.test.ts -t "uses the focused project for screenshot sessions even when the project name differs"`

Expected: FAIL because `screenshotToNewSession` still searches for `"the-controller"` and exits before `create_session`.

**Step 3: Write minimal implementation**

Update `screenshotToNewSession` to call `getTargetProject()` and use that project's `id`. If no focused project exists, show a generic selection error and return.

**Step 4: Run test to verify it passes**

Run the same Vitest command.

Expected: PASS.

**Step 5: Commit**

Commit after broader verification and review are complete.

### Task 2: Verify The App-Level Regression

**Files:**

- Modify: `src/App.svelte`
- Modify: `src/App.test.ts`
- Modify: `docs/plans/2026-03-10-screenshot-session-project-selection-design.md`
- Modify: `docs/plans/2026-03-10-screenshot-session-project-selection.md`

**Step 1: Run broader verification**

Run: `npx vitest run src/App.test.ts`

Expected: PASS.

**Step 2: Self-review**

Check the diff for:

- no remaining hardcoded `"the-controller"` lookup in screenshot flow
- no screenshot capture when no project is selected
- regression test that fails again if the hardcoded lookup is restored

**Step 3: Commit**

Use a commit message body that includes the required trailer:

```text
fix: use focused project for screenshot sessions

closes #248

Contributed-by: auto-worker
```
