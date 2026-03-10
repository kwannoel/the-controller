# Notes Escape Normal Mode Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Make a single `Escape` leave the notes editor only when Vim is already in normal mode.

**Architecture:** Extend the CodeMirror Vim wrapper to surface mode changes to `NotesEditor`, then replace the notes editor's timing-based double-escape logic with a mode-aware focus transition. Cover the change with focused regression tests in `NotesEditor.test.ts`.

**Tech Stack:** Svelte 5, TypeScript, Vitest, Testing Library, CodeMirror 6, `@replit/codemirror-vim`

---

### Task 1: Lock in the new escape behavior with failing tests

**Files:**
- Modify: `src/lib/NotesEditor.test.ts`

**Step 1: Write the failing test**

Replace the old double-escape regression with:
- a test proving one `Escape` in normal mode moves focus to `{ type: "note", ... }`
- a test proving after entering insert mode, one `Escape` keeps focus on `{ type: "notes-editor", ... }`

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/NotesEditor.test.ts`
Expected: FAIL because the current implementation still requires a second `Escape` and does not know the Vim mode.

### Task 2: Surface Vim mode from the editor wrapper

**Files:**
- Modify: `src/lib/CodeMirrorNoteEditor.svelte`

**Step 1: Write minimal implementation**

- Add an `onModeChange` callback prop.
- Use `getCM(view)` from `@replit/codemirror-vim` to subscribe to `vim-mode-change`.
- Track the latest Vim mode and include it when invoking the escape callback.

**Step 2: Run targeted tests**

Run: `npx vitest run src/lib/NotesEditor.test.ts`
Expected: still FAIL until `NotesEditor` consumes the mode.

### Task 3: Replace double-escape timing with mode-aware focus exit

**Files:**
- Modify: `src/lib/NotesEditor.svelte`

**Step 1: Write minimal implementation**

- Remove the timing state used for double-escape detection.
- Track the latest Vim mode reported by `CodeMirrorNoteEditor`.
- On `Escape`, move focus back to the selected note only when the mode is `normal`.

**Step 2: Run tests to verify they pass**

Run: `npx vitest run src/lib/NotesEditor.test.ts`
Expected: PASS

### Task 4: Final verification

**Files:**
- No code changes expected

**Step 1: Run verification**

Run: `npx vitest run src/lib/NotesEditor.test.ts`
Expected: PASS with no failures in the file.
