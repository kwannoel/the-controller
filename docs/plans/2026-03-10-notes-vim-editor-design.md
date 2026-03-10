# Notes Vim Editor Design

## Definition

Replace the notes workspace textarea with an in-app markdown editor that supports Vim bindings, preserves markdown preview, and keeps note editing integrated with the rest of the app's focus and keyboard model.

## Constraints

- Keep note storage as plain `.md` files under the existing notes storage layout.
- Preserve current note loading and autosave behavior rather than redesigning persistence.
- Keep notes integrated with sidebar focus and app-level navigation.
- Vim mode is always enabled for notes; there is no per-user toggle in this iteration.
- Support three note display modes: `Edit`, `Preview`, and `Split`.
- Preserve a keyboard path to leave the editor and return focus to the selected note item in the sidebar.
- Treat this as a behavior change and implement it with TDD before production code.
- Favor an editor foundation that will support future selection-based AI actions on note text.

## Approaches

### 1. Extend the existing textarea with handwritten Vim behavior

Add modal key handling on top of the textarea and manually implement the subset of Vim interactions the app needs.

Pros: Minimal dependency changes, smallest immediate code delta.
Cons: Becomes brittle quickly, incomplete relative to user expectations, and creates a dead-end for richer editor behavior.

### 2. Replace the textarea with CodeMirror 6 plus Vim bindings

Use CodeMirror 6 as the in-app markdown editor, enable Vim keybindings, and integrate editor focus with the app's existing note and sidebar state.

Pros: Mature editor model, practical Vim support, explicit markdown support, and a clean path for future selection-aware AI actions.
Cons: Adds frontend dependencies and requires adapting editor lifecycle and test setup.

### 3. Hand notes off to an external Vim process

Open the note file in `vim` or `nvim` outside the app and let the app observe file changes.

Pros: Real Vim behavior with minimal in-app editor work.
Cons: Breaks the integrated notes workflow and blocks future in-app selection-aware AI actions.

## Chosen Design

Use approach 2. Replace the note editor textarea in `src/lib/NotesEditor.svelte` with a CodeMirror 6 editor configured for markdown and Vim bindings. Keep the current note file model and autosave flow intact so the rest of the notes feature continues to operate on plain markdown files.

Expand note view state from a boolean preview flag to an explicit notes view mode with `edit`, `preview`, and `split` values. `Edit` shows only the editor, `Preview` shows only rendered markdown, and `Split` shows the editor and preview side by side. The header should expose direct buttons for all three modes, while the existing notes hotkey can cycle through them for keyboard use.

Editor focus stays inside CodeMirror for normal typing and Vim commands. Single-key global hotkeys should remain suppressed while the editor is focused so Vim input is not hijacked by the app. To preserve navigation back to the sidebar, the notes editor should detect a double-`Escape` sequence and move focus to the currently selected note item in the sidebar. Single `Escape` remains available to Vim for mode switching.

This design preserves the app's current note CRUD, focus tracking, and markdown rendering boundaries while upgrading the editing surface to something that can later support selection-based AI transformations.

## Validation

- Add failing store or command coverage for the new explicit notes view mode behavior.
- Add failing component tests for `src/lib/NotesEditor.svelte` covering `Edit`, `Preview`, and `Split` modes.
- Add failing component tests proving notes still load and autosave through the new editor integration.
- Add failing interaction tests proving double `Escape` from the focused notes editor returns focus to the selected note item, while single-key hotkeys remain suppressed during editor focus.
- Run focused frontend tests in a failing state first, then rerun after implementation until they pass.
- Run the full frontend test suite before claiming the feature is complete.
