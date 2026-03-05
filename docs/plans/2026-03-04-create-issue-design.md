# Create GitHub Issue via `i` Key (#21)

## Summary

Press `i` to create a GitHub issue for the focused project. A modal collects the title, Claude generates the body, `gh` creates the issue, and it appears instantly in the task panel.

## Flow

1. User presses `i` in ambient mode
2. If no project/session is focused, no-op
3. Modal appears with a single title input
4. User types title, presses Enter
5. Backend:
   - Extracts GitHub remote via `parse_github_nwo` (reused from task panel)
   - Generates structured body via `claude -p` from the title
   - Creates issue via `gh issue create --repo <nwo> --title --body --json number,title,url,labels`
   - Returns created `GithubIssue` (correct number from GitHub)
6. Frontend opens task panel, optimistically inserts the new issue at the top
7. Background refresh syncs with GitHub

## Backend: `create_github_issue` Tauri Command

- Input: `repo_path: String`, `title: String`
- Async + `spawn_blocking` for git2, `tokio::process::Command` for `claude` and `gh`
- Returns: `GithubIssue` (reuse existing model)

## Frontend

- **CreateIssueModal.svelte** — single title input, Enter to submit, Escape to cancel
- **HotkeyAction** — `{ type: "create-issue", projectId: string }`
- **`i` key** in HotkeyManager dispatches action when focused
- **TaskPanel** — method to optimistically insert an issue; panel auto-opens on creation
