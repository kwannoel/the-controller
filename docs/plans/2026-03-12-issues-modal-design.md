# Issues Modal Revamp

## Problem

Issue interactions are scattered across 4 separate entry points: `CreateIssueModal` (i), `IssuePickerModal` (c → pick), `TriagePanel` (t/T), and `AssignedIssuesPanel` (e). This revamp consolidates them into a single keyboard-driven modal.

## Design

### Entry Point

- `i` in development mode opens the issues modal (scoped to focused project's repo)
- `c` in the sidebar now spawns a session directly (no issue picker)

### State Machine

Single `IssuesModal.svelte` with a `view` state:

```
hub ──c──▶ create (title → priority → complexity)
 │
 f──▶ find (search input + filtered list | detail pane)
              │
              a──▶ assigns selected issue to new session
```

### Hub View

Centered modal with standard overlay. Shows:
- Header: "Issues" with repo name
- Two menu options: `c` Create issue, `f` Find issues

### Create View

Same multi-stage flow as current `CreateIssueModal`: title input → priority (j/k) → complexity (j/k). Escape returns to hub.

### Find View (Split Layout)

- **Left pane:** Search input (auto-focused) + filtered issue list below. Case-insensitive substring match across title, body, and label names. `j/k` navigate the list.
- **Right pane:** Detail view of highlighted issue — number, title, body, labels, assignees. Updates as user navigates.
- `a` on highlighted issue dispatches `pick-issue-for-session` (assigns issue to new session)
- `Enter` opens issue in browser

### Escape Hierarchy

- `create` → `hub`
- `find` (with search text) → clears search
- `find` (empty search) → `hub`
- `hub` → closes modal

### Keybinding Changes

| Key | Before | After |
|-----|--------|-------|
| `i` | `create-issue` | `open-issues-modal` |
| `c` | `create-session` (with issue picker) | `create-session` (direct, no picker) |
| `t` | `triage-untriaged` | freed |
| `T` | `triage-triaged` | freed |
| `e` | `assigned-issues` | freed |

### Files Removed

- `CreateIssueModal.svelte` — absorbed into IssuesModal
- `IssuePickerModal.svelte` — replaced by find view
- `TriagePanel.svelte` — replaced by find view
- `AssignedIssuesPanel.svelte` — replaced by find view

### Props

```typescript
interface Props {
  repoPath: string;
  projectId: string;
  onClose: () => void;
  onCreateIssue: (title: string, priority: Priority, complexity: Complexity) => void;
  onAssignIssue: (issue: GithubIssue) => void;
}
```
