# GitHub Task List Panel (Issue #12)

## Summary

Add a toggleable RHS panel showing open GitHub issues for the focused project. Toggled via `t` key in ambient mode.

## Architecture

### Layout

Extend the two-panel flex layout (sidebar + terminal) to a three-panel layout with an optional RHS task panel. The panel appears as a third flex child in `App.svelte`, conditionally rendered based on a `taskPanelVisible` store.

### New Component: `TaskPanel.svelte`

- Renders open GitHub issues (number, title) in a scrollable list
- Loading state while fetching
- Error state if `gh` CLI fails or repo has no remote
- Re-fetches when focused project changes (context-aware)
- Fetches issues each time the panel is toggled open

### State

- New store: `taskPanelVisible: writable<boolean>` (default `false`)
- Panel reads the focused project's `repo_path` to determine which repo to query

### Backend: `list_github_issues` Tauri Command

- Input: `repo_path: String`
- Extracts GitHub remote URL from the repo (origin remote)
- Runs: `gh issue list --json number,title,labels,url --limit 50`
- Must be `async` + `spawn_blocking` (shells out to `gh`)
- Returns: `Vec<GithubIssue>` with fields: `number`, `title`, `url`, `labels`

### Keybinding

- `t` in ambient mode toggles `taskPanelVisible`
- Added to `HotkeyManager.svelte` handleHotkey dispatch
- Added to `HotkeyHelp.svelte` help table

### Styling

- Fixed width (~320px), Catppuccin Mocha theme
- Left border matching sidebar's right border style (`1px solid #313244`)
- Issue numbers in accent color (`#89b4fa`), titles in foreground
- Scrollable issue list
