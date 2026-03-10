# Provider Toggle Session Hotkeys Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Simplify session hotkeys so `c` is the only foreground creation command, `Cmd+T` toggles the selected foreground provider between Claude and Codex, background issue execution always uses Codex, and the sidebar footer shows the active foreground provider.

**Architecture:** Add a small app-level store for the selected foreground provider and thread it through the existing hotkey-to-action flow. Keep background creation stateless by always dispatching/creating Codex background sessions, while removing the old `x`, `X`, and `C` command registry entries and help text. Expose the selected provider in the sidebar footer and handle `Cmd+T` in the external hotkey path.

**Tech Stack:** Svelte 5 runes, Svelte stores, Vitest, Testing Library

---

### Task 1: Add failing command-registry coverage

**Files:**
- Modify: `src/lib/commands.test.ts`
- Modify: `src/lib/commands.ts`

**Step 1: Write the failing test**

Add assertions that development mode exposes `c` but not `x`, `X`, or `C`, and that help text no longer lists the removed commands.

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/commands.test.ts`
Expected: FAIL because the old command ids and keys still exist.

**Step 3: Write minimal implementation**

Remove the split foreground/background provider command ids from `src/lib/commands.ts`, replace them with a single `create-session` command on `c`, and leave background creation out of the registry.

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/commands.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add src/lib/commands.ts src/lib/commands.test.ts
git commit -m "refactor: simplify session hotkey registry"
```

### Task 2: Add failing hotkey/provider-state coverage

**Files:**
- Modify: `src/lib/HotkeyManager.test.ts`
- Modify: `src/lib/stores.ts`
- Modify: `src/lib/HotkeyManager.svelte`

**Step 1: Write the failing test**

Add tests that prove:
- `c` dispatches `pick-issue-for-session` with the currently selected foreground provider
- `Cmd+T` toggles the selected foreground provider
- `Cmd+T` is ignored while an editable element is focused
- the removed `x`, `X`, and `C` keys no longer dispatch anything

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/HotkeyManager.test.ts`
Expected: FAIL because no selected-provider store exists and old keys still dispatch actions.

**Step 3: Write minimal implementation**

Add a `selectedSessionProvider` store, teach `HotkeyManager.svelte` to use it for `c`, and add an external `Cmd+T` handler that toggles only the foreground provider state.

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/HotkeyManager.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add src/lib/stores.ts src/lib/HotkeyManager.svelte src/lib/HotkeyManager.test.ts
git commit -m "feat: add provider toggle hotkey"
```

### Task 3: Add failing app/sidebar behavior coverage

**Files:**
- Modify: `src/App.test.ts`
- Modify: `src/lib/Sidebar.svelte`
- Modify: `src/App.svelte`

**Step 1: Write the failing test**

Add tests that prove:
- skipping issue picker creates a session with the selected foreground provider
- background issue sessions always create Codex sessions regardless of selected foreground provider
- the sidebar footer renders the active provider indicator text

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/App.test.ts`
Expected: FAIL because the app still defaults foreground creation to Claude and the sidebar has no provider indicator.

**Step 3: Write minimal implementation**

Update `App.svelte` to resolve the selected provider when creating normal sessions and keep background issue creation pinned to Codex. Render the active provider text in `Sidebar.svelte`.

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/App.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add src/App.svelte src/App.test.ts src/lib/Sidebar.svelte
git commit -m "feat: show active provider and pin background codex"
```

### Task 4: Final verification

**Files:**
- Modify: `src/lib/HotkeyHelp.svelte` only if tests or help snapshots require text updates

**Step 1: Run targeted verification**

Run: `npx vitest run src/lib/commands.test.ts src/lib/HotkeyManager.test.ts src/App.test.ts`
Expected: PASS

**Step 2: Run broader frontend verification**

Run: `npx vitest run`
Expected: PASS

**Step 3: Sanity-check removed help text**

Run: `rg -n "\"x\"|\"X\"|\"C\"|create-session-codex|background-worker-claude|background-worker-codex" src/lib`
Expected: no stale session-hotkey references outside intentional historical tests or unrelated literals

**Step 4: Commit**

```bash
git add docs/plans/2026-03-10-provider-toggle-session-hotkeys.md
git commit -m "docs: add provider toggle hotkey plan"
```
