# Notes Vim Editor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Replace the notes textarea with a Vim-enabled markdown editor that supports edit, preview, and split modes while preserving note autosave and sidebar focus integration.

**Architecture:** Keep note persistence and markdown rendering in the existing notes workspace, but swap the editor surface to CodeMirror 6 with Vim bindings. Promote note view mode from a boolean preview flag to an explicit enum-like store so the UI and hotkeys can support `Edit`, `Preview`, and `Split`, then wire a double-`Escape` out-focus path that returns to the selected note row.

**Tech Stack:** Svelte 5, TypeScript, Vitest, CodeMirror 6, `@replit/codemirror-vim`

---

### Task 1: Add the editor dependencies

**Files:**
- Modify: `package.json`
- Modify: `package-lock.json`

**Step 1: Add the packages**

Add the minimal editor packages needed for:
- CodeMirror state and view
- Markdown language support
- Vim bindings

Expected packages:
- `@codemirror/state`
- `@codemirror/view`
- `@codemirror/lang-markdown`
- `@replit/codemirror-vim`

**Step 2: Install dependencies**

Run: `npm install`
Expected: lockfile updates with the new editor packages.

**Step 3: Commit**

```bash
git add package.json package-lock.json
git commit -m "chore(notes): add codemirror vim dependencies"
```

### Task 2: Write failing tests for notes view modes

**Files:**
- Modify: `src/lib/NotesEditor.test.ts`
- Modify: `src/lib/stores.ts`
- Test: `src/lib/NotesEditor.test.ts`

**Step 1: Write the failing tests**

Add tests that prove:
- `Edit` mode shows only the editor surface.
- `Preview` mode shows only rendered markdown.
- `Split` mode shows both the editor surface and rendered markdown.
- header controls switch among the three modes.

Use stable assertions such as:
- visible text labels for mode buttons
- presence or absence of the preview container
- presence or absence of the editor container

**Step 2: Run the focused test to verify it fails**

Run: `npx vitest run src/lib/NotesEditor.test.ts`
Expected: FAIL because the current implementation only supports a boolean preview toggle.

### Task 3: Replace boolean preview state with explicit view mode

**Files:**
- Modify: `src/lib/stores.ts`
- Modify: `src/lib/commands.ts`
- Modify: `src/lib/commands.test.ts`
- Test: `src/lib/commands.test.ts`

**Step 1: Write the failing command/store tests**

Add tests covering:
- notes commands still expose `toggle-note-preview` in Notes mode
- the new state model supports cycling among `edit`, `preview`, and `split`

If `commands.test.ts` is the cleanest place to anchor behavior, add the assertions there and keep the store type simple.

**Step 2: Run the focused test to verify it fails**

Run: `npx vitest run src/lib/commands.test.ts`
Expected: FAIL because the notes view state is still boolean and cannot represent `split`.

**Step 3: Write the minimal implementation**

Change `src/lib/stores.ts` so note view state becomes an explicit string union such as:
- `edit`
- `preview`
- `split`

Update command-facing code as needed so the notes hotkey can continue to target a single action that cycles modes.

**Step 4: Run the focused test to verify it passes**

Run: `npx vitest run src/lib/commands.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add src/lib/stores.ts src/lib/commands.ts src/lib/commands.test.ts
git commit -m "refactor(notes): use explicit editor view modes"
```

### Task 4: Add failing tests for focus handoff and hotkey suppression

**Files:**
- Modify: `src/lib/NotesEditor.test.ts`
- Modify: `src/lib/HotkeyManager.test.ts`
- Test: `src/lib/NotesEditor.test.ts`
- Test: `src/lib/HotkeyManager.test.ts`

**Step 1: Write the failing tests**

Add coverage proving:
- single-key hotkeys do not fire while the notes editor surface is focused
- a single `Escape` inside the notes editor does not move focus away
- a double `Escape` inside the notes editor returns focus to the selected note item

Model the double-escape timing using the same timeout assumptions the app already uses elsewhere.

**Step 2: Run the focused tests to verify they fail**

Run: `npx vitest run src/lib/NotesEditor.test.ts src/lib/HotkeyManager.test.ts`
Expected: FAIL because the current editor and hotkey logic do not support CodeMirror-backed focus and double-escape out-focus behavior.

### Task 5: Build a reusable CodeMirror-backed notes editor component

**Files:**
- Create: `src/lib/CodeMirrorNoteEditor.svelte`
- Modify: `src/lib/NotesEditor.test.ts`
- Test: `src/lib/NotesEditor.test.ts`

**Step 1: Write the minimal implementation**

Create a dedicated wrapper component that:
- mounts a CodeMirror editor into a container element
- accepts note content as input
- emits content changes for autosave
- enables markdown support
- enables Vim bindings
- exposes a stable DOM hook for tests to detect the editor surface

Keep this component narrowly scoped to editor lifecycle and content synchronization.

**Step 2: Run the focused notes editor test to verify progress**

Run: `npx vitest run src/lib/NotesEditor.test.ts`
Expected: still FAIL on higher-level integration assertions until `NotesEditor.svelte` is updated, but editor-surface assertions should start becoming satisfiable.

### Task 6: Integrate CodeMirror into the notes workspace

**Files:**
- Modify: `src/lib/NotesEditor.svelte`
- Modify: `src/lib/NotesEditor.test.ts`
- Test: `src/lib/NotesEditor.test.ts`

**Step 1: Write the minimal implementation**

Update `NotesEditor.svelte` to:
- use the new explicit notes view mode store
- render `Edit`, `Preview`, and `Split` buttons
- render `CodeMirrorNoteEditor` for `edit` and `split`
- keep markdown preview rendering for `preview` and `split`
- preserve note loading and autosave behavior
- preserve active note title and unsaved state display

**Step 2: Run the focused notes editor test to verify it passes**

Run: `npx vitest run src/lib/NotesEditor.test.ts`
Expected: PASS

**Step 3: Commit**

```bash
git add src/lib/CodeMirrorNoteEditor.svelte src/lib/NotesEditor.svelte src/lib/NotesEditor.test.ts
git commit -m "feat(notes): add codemirror vim notes editor"
```

### Task 7: Wire double-escape out-focus and app hotkey compatibility

**Files:**
- Modify: `src/lib/NotesEditor.svelte`
- Modify: `src/lib/HotkeyManager.svelte`
- Modify: `src/lib/HotkeyManager.test.ts`
- Modify: `src/lib/NotesEditor.test.ts`
- Test: `src/lib/HotkeyManager.test.ts`
- Test: `src/lib/NotesEditor.test.ts`

**Step 1: Write the minimal implementation**

Update editor/app interaction so:
- the notes editor can detect double `Escape`
- a double `Escape` moves focus to `{ type: "note", ... }` for the active note
- normal editor keystrokes do not leak into global notes hotkeys while CodeMirror is focused
- the existing app double-escape behavior outside the notes editor remains intact

Prefer a clear integration boundary instead of scattering special cases across unrelated code paths.

**Step 2: Run the focused interaction tests to verify they pass**

Run: `npx vitest run src/lib/NotesEditor.test.ts src/lib/HotkeyManager.test.ts`
Expected: PASS

**Step 3: Commit**

```bash
git add src/lib/NotesEditor.svelte src/lib/HotkeyManager.svelte src/lib/HotkeyManager.test.ts src/lib/NotesEditor.test.ts
git commit -m "feat(notes): preserve sidebar focus with vim editor"
```

### Task 8: Verify the full notes workflow

**Files:**
- Modify: `src/lib/commands.test.ts`
- Modify: `src/lib/NotesEditor.test.ts`
- Modify: `src/lib/HotkeyManager.test.ts`
- Test: `src/lib/commands.test.ts`
- Test: `src/lib/NotesEditor.test.ts`
- Test: `src/lib/HotkeyManager.test.ts`

**Step 1: Run the focused notes-related suite**

Run: `npx vitest run src/lib/commands.test.ts src/lib/NotesEditor.test.ts src/lib/HotkeyManager.test.ts`
Expected: PASS

**Step 2: Run broader frontend verification**

Run: `npx vitest run`
Expected: PASS

**Step 3: Review the diff for scope**

Check that:
- note files are still plain markdown files
- notes hotkeys still work in Notes mode
- the editor supports `Edit`, `Preview`, and `Split`
- double `Escape` returns focus to the selected note item
- no development or agents workspace behavior regressed

**Step 4: Commit**

```bash
git add src/lib/commands.test.ts src/lib/NotesEditor.test.ts src/lib/HotkeyManager.test.ts
git commit -m "test(notes): verify vim editor integration"
```
