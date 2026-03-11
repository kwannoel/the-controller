# Auto-Worker Eligible Issues Panel

## Problem

The auto-worker panel currently shows only a coarse status message and completed reports. It does not show which issues are eligible to be picked next, so users cannot inspect the worker's queue or understand why the worker is idle without leaving the panel.

## Design

Add a navigable `Queue` view to the auto-worker panel that shows the currently active issue plus all other eligible issues for the project. Keep completed work in the existing `Reports` view and let the panel toggle between the two views.

### Interaction

- Auto-worker panel gets two views: `Queue` and `Reports`.
- `Queue` is the default view when the auto-worker panel is focused.
- In `Queue` list view, `j` / `k` move selection, `l` opens the selected issue, and `Esc` backs out.
- The issue currently being worked appears in the queue and is marked `Working`.
- Remaining eligible issues appear below it as queued candidates.
- Opening an item shows its issue detail in-panel and supports the existing open-in-browser action.

### Data Flow

- Add a Tauri command that returns the auto-worker queue for a project.
- The backend command reuses the Rust auto-worker eligibility predicate so the scheduler and UI always agree on which issues are eligible.
- The queue payload includes: `number`, `title`, `url`, `labels`, `body`, and `is_active`.
- The frontend fetches the queue when the focused agent becomes `auto-worker`.
- The queue refreshes when auto-worker status changes so the panel follows claim / completion transitions without manual refresh.

### States

- Empty queue: show `No eligible issues`.
- Loading queue data is independent from report loading so switching views does not blank unrelated content.
- Queue fetch failures show a toast and fall back to an empty list.
- When the worker is active, its issue is pinned into the queue response even if its labels no longer satisfy the eligible predicate after claim time.
- When the active issue finishes, it drops out of the queue and remains visible only in completed reports.

### Scope

In scope:
- New backend command for auto-worker queue data
- Auto-worker queue list and issue detail UI
- Auto-worker panel view toggle between queue and reports
- Keyboard navigation and browser-open support for the queue

Out of scope:
- Editing issues from the queue
- Reordering or prioritizing the queue in the UI
- A separate project-wide issue browser for auto-worker
