# Screenshot Session Provider Design

## Summary

`screenshotToNewSession` in `src/App.svelte` currently hardcodes `kind: "claude"` when it creates a new session from a screenshot. That bypasses the selected foreground provider state introduced for other session-entry paths, so screenshot flows do not follow the current provider toggle.

## Goals

- Make every screenshot shortcut create the new session with the current foreground provider.
- Keep screenshot capture, preview, and focused-project behavior unchanged.
- Add regression coverage that fails if screenshot session creation is hardcoded to Claude again.

## Non-Goals

- Changing screenshot capture or preview behavior.
- Changing background issue execution, which should remain Codex-only.
- Refactoring unrelated session creation code.

## Approach Options

### Option 1: Reuse the current foreground provider state

Pros:

- Matches normal foreground session creation behavior.
- Minimal change with clear regression coverage.
- Keeps provider selection centralized in the existing store.

Cons:

- Screenshot sessions continue to depend on app-level provider state.

### Option 2: Keep screenshots pinned to Claude

Pros:

- No implementation work.

Cons:

- Conflicts with the current provider toggle model.
- Surprises users because screenshot sessions behave differently from other foreground sessions.

### Option 3: Add a separate screenshot provider setting

Pros:

- Explicit per-feature control.

Cons:

- Adds state and UI complexity without a clear need.
- Creates another provider rule to learn.

Recommendation: Option 1.

## Design

Update `screenshotToNewSession` to use `currentSessionProvider` for the `create_session.kind` field instead of a hardcoded `"claude"`. This makes all screenshot entry points (`Cmd+S`, `Cmd+Shift+S`, `Cmd+D`, `Cmd+Shift+D`) follow the same foreground provider state as the `c` session flow.

The project lookup, capture command, preview behavior, and initial prompt remain unchanged.

## Testing

- Add an app-level regression test that sets `selectedSessionProvider` to `"codex"`, triggers screenshot-to-session, and verifies `create_session` receives `kind: "codex"`.
- Run that targeted test first and confirm it fails against the hardcoded implementation.
- Implement the minimal `App.svelte` change.
- Re-run the targeted test and then the `src/App.test.ts` suite to confirm the screenshot variants still work.
