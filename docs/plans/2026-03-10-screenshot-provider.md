# Screenshot Session Provider Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Make screenshot-created sessions use the current foreground provider instead of always creating Claude sessions.

**Architecture:** Reuse the existing `selectedSessionProvider` state already threaded through normal foreground session creation. Constrain the change to `App.svelte` and protect it with an app-level regression test in `src/App.test.ts`.

**Tech Stack:** Svelte 5, Svelte stores, Vitest, Testing Library

---

### Task 1: Add Regression Coverage For Screenshot Provider Selection

**Files:**
- Modify: `src/App.test.ts`
- Test: `src/App.test.ts`

**Step 1: Write the failing test**

Add a test that renders `App`, sets `selectedSessionProvider` to `"codex"`, triggers `hotkeyAction` with `type: "screenshot-to-session"`, and asserts `invoke("create_session", ...)` receives `kind: "codex"`.

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/App.test.ts -t "uses the selected provider for screenshot sessions"`

Expected: FAIL because `screenshotToNewSession` still hardcodes `kind: "claude"`.

**Step 3: Write minimal implementation**

Update `screenshotToNewSession` to use `currentSessionProvider` when it calls `create_session`.

**Step 4: Run test to verify it passes**

Run the same Vitest command.

Expected: PASS.

**Step 5: Commit**

Commit after broader verification is complete.

### Task 2: Verify Screenshot Flow Regressions

**Files:**
- Modify: `src/App.svelte`
- Modify: `src/App.test.ts`
- Modify: `docs/plans/2026-03-10-screenshot-provider-design.md`
- Modify: `docs/plans/2026-03-10-screenshot-provider.md`

**Step 1: Run broader verification**

Run: `npx vitest run src/App.test.ts`

Expected: PASS.

**Step 2: Self-review**

Check the diff for:

- screenshot sessions using `currentSessionProvider`
- unchanged screenshot preview and capture behavior
- regression coverage that fails again if `kind: "claude"` is restored

**Step 3: Commit**

Use a concise fix commit message after verification.
