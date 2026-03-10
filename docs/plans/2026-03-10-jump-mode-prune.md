# Jump Mode Prune Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Remove the obsolete `g` jump-mode entrypoint and all unused jump-mode plumbing from the development sidebar hotkey system.

**Architecture:** Delete the command-registry entry, the hotkey manager's jump-mode state machine, and the sidebar/tree/store helpers that only exist to render and drive jump labels. Update tests to assert the new behavior and remove tests that only covered deleted functionality.

**Tech Stack:** Svelte 5, Vitest, Testing Library

---

### Task 1: Make the behavior change fail first

**Files:**
- Modify: `src/lib/commands.test.ts`
- Modify: `src/lib/HotkeyManager.test.ts`
- Modify: `src/lib/stores.test.ts`

**Step 1: Write the failing test**

- Remove assertions that expect `g`/jump mode to exist.
- Add assertions that `g` is absent from the command registry and does nothing in ambient mode.
- Remove store tests that validate jump-mode-specific exports.

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/commands.test.ts src/lib/HotkeyManager.test.ts src/lib/stores.test.ts`

Expected: FAIL because implementation still exports and handles jump mode.

### Task 2: Remove jump-mode implementation and dead UI props

**Files:**
- Modify: `src/lib/commands.ts`
- Modify: `src/lib/HotkeyManager.svelte`
- Modify: `src/lib/stores.ts`
- Modify: `src/lib/Sidebar.svelte`
- Modify: `src/lib/sidebar/ProjectTree.svelte`
- Modify: `src/lib/sidebar/ProjectTree.test.ts`
- Modify: `src/lib/Sidebar.test.ts`

**Step 1: Write minimal implementation**

- Delete the `jump-mode` command id and `g` command definition.
- Delete jump-mode state, handlers, and event interception from the hotkey manager.
- Delete jump-mode store exports and any sidebar/tree props derived solely from them.
- Remove jump-mode setup from affected tests.

**Step 2: Run test to verify it passes**

Run: `npx vitest run src/lib/commands.test.ts src/lib/HotkeyManager.test.ts src/lib/stores.test.ts src/lib/Sidebar.test.ts src/lib/sidebar/ProjectTree.test.ts`

Expected: PASS

### Task 3: Final targeted verification

**Files:**
- No additional code changes expected

**Step 1: Run focused regression suite**

Run: `npx vitest run src/lib/commands.test.ts src/lib/HotkeyManager.test.ts src/lib/stores.test.ts src/lib/Sidebar.test.ts src/lib/sidebar/ProjectTree.test.ts`

Expected: PASS with no jump-mode references remaining in active code paths.
