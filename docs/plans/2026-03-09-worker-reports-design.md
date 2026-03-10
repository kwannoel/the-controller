# Worker Issue Reports

## Problem

The auto-worker completes issues autonomously but leaves no trace of what it did. There's no way to see a summary of its work from the dashboard, and the `finished-by-worker` label gets applied even when the PR wasn't actually merged.

## Design

Two parts: (1) the worker posts a report comment on the GitHub issue, and (2) the dashboard displays those reports.

### Invariant

For every issue labeled `finished-by-worker`, there must be an associated PR that is actually merged. Both the label and the report comment are only applied/posted when a merged PR exists.

### Part 1: Worker posts report comment

**Prompt change** in `session_args.rs` — add a step between "Merge" and "Sync local master":

> Post a report comment on the issue via `gh issue comment <number> --body "..."`.
> The report should summarize: what was changed, what PR was created, and whether the merge succeeded.
> Only post the report after the PR has been successfully merged.

### Part 2: Scheduler enforces the invariant

**In `auto_worker.rs`** — before applying `finished-by-worker`, verify a merged PR exists:

```bash
gh pr list --search "closes #N" --state merged --json number --limit 1
```

- If a merged PR exists: apply `finished-by-worker` label, remove `in-progress`
- If no merged PR: remove `in-progress` only, do not apply `finished-by-worker`

This replaces the current `mark_issue_finished` which unconditionally applies the label when the PTY exits.

### Part 3: Dashboard displays reports

**New Tauri command** `get_worker_reports`:
- Runs `gh issue list --label finished-by-worker --json number,title,comments --state all` for the project's repo
- Extracts the latest comment body from each issue
- Returns `Vec<WorkerReport>` with `{issue_number, title, comment_body, updated_at}`

**Frontend** in `AgentDashboard.svelte`:
- When auto-worker panel is focused, fetches reports via `get_worker_reports` (same pattern as maintainer's `fetchHistory`)
- Displays as a navigable list of reports (j/k to select, enter to open, escape to go back)
- Each report shows: issue number, title, and the worker's comment
- Reuses existing navigation infrastructure (selectedIndex, openLogIndex, detail view)

### Data flow

```
Worker agent (in PTY)
  ├─ Creates PR, merges it
  ├─ Posts report comment: gh issue comment #N --body "..."
  └─ PTY exits

Scheduler (auto_worker.rs)
  ├─ Detects PTY exit
  ├─ Checks: gh pr list --search "closes #N" --state merged
  ├─ If merged: add finished-by-worker label
  └─ Cleanup session + worktree

Dashboard (AgentDashboard.svelte)
  ├─ On focus: invoke get_worker_reports
  ├─ Fetches: gh issue list --label finished-by-worker (with comments)
  └─ Displays report list with navigation
```

### No new local storage

GitHub is the source of truth. The `finished-by-worker` label acts as the index, and issue comments hold the report content. No new models or persistence layer needed on the Rust side beyond the command return type.
