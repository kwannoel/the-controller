# Design: Assign GitHub Issue to a Session (#22)

## Summary

Support assigning a GitHub issue to a session at creation time, linking the session's work to a specific issue.

Part of the issue lifecycle: create -> **assign** -> merge

## UX

- `c` (lowercase): Opens an issue picker modal, user selects an issue, session spawns linked to it.
- `C` (uppercase): Spawns a raw session with no issue attached (current behavior).
- Linked sessions display the issue badge (e.g., `#22`) next to the session name in the sidebar.

## Behavior

### On assign (session creation)

- A comment is posted on the GitHub issue: "Working on this in session `session-N`"
- The issue number and title are stored in `SessionConfig`
- The issue context (title, body) is passed to the Claude session so it knows what to work on

### On merge (close)

- Handled by GitHub's native "closes #N" convention in commit messages / PR descriptions
- The session's Claude instance is aware of the issue number via context injection, so it can include "closes #N" naturally

## Components

### Backend (Rust)

- **`models.rs`**: Add `github_issue: Option<GithubIssue>` to `SessionConfig`
- **`commands.rs`**: Add `post_github_comment(repo_path, issue_number, body)` Tauri command
- Issue association persists across app restarts (stored in session config)

### Frontend (Svelte)

- **`IssuePickerModal.svelte`** (new): Modal listing open issues for the focused project. User clicks to select. Uses existing `list_github_issues` command.
- **`HotkeyManager.svelte`**: `c` opens issue picker (when project focused), `C` spawns raw session.
- **`Sidebar.svelte`**: Show issue badge next to session name for linked sessions.
- **`App.svelte`**: Orchestrate issue picker flow -> session creation -> comment posting.

### Context injection

- When spawning a session linked to an issue, include the issue title and body in the initial context so Claude knows what it's working on.

## Not in scope

- Summary comments on session idle/archive
- Auto-closing issues from the app (rely on GitHub's native "closes #N")
- Search/filtering in the issue picker (future enhancement)
