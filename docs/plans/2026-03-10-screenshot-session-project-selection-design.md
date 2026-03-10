# Screenshot Session Project Selection Design

## Summary

`screenshotToNewSession` in `src/App.svelte` currently looks up a project named `"the-controller"` before creating the screenshot analysis session. That makes the feature fail in any workspace where the selected project has a different name, even though the rest of the app already tracks the active project through `focusTarget`.

## Goals

- Make screenshot sessions target the project implied by the current UI focus.
- Keep the existing hotkeys and screenshot capture flow unchanged.
- Add regression coverage that fails if screenshot session creation falls back to a hardcoded project name again.

## Non-Goals

- Adding a project picker modal for screenshots.
- Changing stage-in-place behavior or other project-specific hotkeys.
- Refactoring unrelated focus-management code.

## Approach Options

### Option 1: Use the currently focused project

Pros:

- Reuses the app's existing project-selection model.
- Matches the issue's "currently active/selected project" guidance.
- Minimal change with clear regression coverage.

Cons:

- Screenshot flow still depends on a project being focused somewhere in the UI.

### Option 2: Fall back to the first available project when nothing is focused

Pros:

- Avoids an error toast when the user has projects loaded but no current focus.

Cons:

- Implicit target selection is surprising.
- Risks sending screenshots to the wrong project silently.

### Option 3: Add a target-project picker for screenshots

Pros:

- Explicit and flexible.

Cons:

- Larger UI change than this issue requires.
- Slower flow for a hotkey-driven action.

Recommendation: Option 1.

## Design

Resolve the screenshot target project via the existing `getTargetProject()` helper, which reads `focusTarget` and maps it back to a project in `projectsState.current`. If there is no focused project context, show a generic error toast like `Select a project before starting a screenshot session` and exit before any screenshot capture occurs.

This keeps the rule simple: screenshot sessions run in whichever project the user is actively working in, whether focus is on the project row itself or one of its child targets.

## Testing

- Add a regression test that sets the only project name to something other than `"the-controller"`, keeps focus on that project, triggers the screenshot hotkey, and verifies `create_session` receives that project's id.
- Run the targeted test first and confirm it fails against the current hardcoded lookup.
- Implement the minimal `App.svelte` change.
- Re-run the targeted test plus the `src/App.test.ts` suite to confirm the regression stays covered.
