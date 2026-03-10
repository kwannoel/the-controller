# Notes Escape Normal Mode Design

## Definition

Refine notes-editor Vim integration so `Escape` behaves like a true "back out" key: when the editor is already in Vim normal mode, a single `Escape` should leave the editor and return focus to the selected note row in the sidebar.

## Constraints

- Preserve standard Vim behavior for insert and visual mode transitions.
- Keep notes editing inside the current CodeMirror + `@replit/codemirror-vim` implementation.
- Do not route editor keystrokes back through the global hotkey manager while the editor is focused.
- Remove the timing-based double-escape requirement from notes mode.
- Prove the behavior with regression tests before implementation changes.

## Chosen Design

Track the editor's current Vim mode inside the CodeMirror wrapper by subscribing to the plugin's `vim-mode-change` signal. Pass that mode to `NotesEditor` alongside `Escape` key events. `NotesEditor` should move focus back to the selected note only when it receives `Escape` while the editor reports Vim normal mode. In insert, replace, or visual modes, `Escape` should remain editor-local so Vim can handle its normal mode transition.

This keeps the app aligned with user intent: the first `Escape` exits edit submodes, and once the editor is already at the top-level Vim state, the next `Escape` exits back to sidebar navigation.

## Validation

- Add a failing notes-editor test proving `Escape` in normal mode moves focus to the selected note row immediately.
- Add a failing notes-editor test proving `Escape` in insert mode does not move focus out of the editor.
- Run the focused notes-editor tests red, implement the smallest change, then rerun green.
- Run the full notes-editor test file after implementation.
