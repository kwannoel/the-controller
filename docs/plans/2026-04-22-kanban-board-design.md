# Kanban Board for Tracking Issues (Issue #537)

## Definition

A full-screen kanban board view for visualising and moving GitHub issues across lifecycle columns. Cards are dragged between columns to update status; status is persisted on GitHub as a `status:*` label.

## Constraints

- **Status source:** GitHub labels (`status:backlog`, `status:todo`, `status:in-progress`, `status:in-review`, `status:done`). No GitHub Projects v2 integration, no local status store.
- **UI placement:** New full-screen view, reachable via a hotkey. Sits alongside `TerminalManager` / `AgentDashboard` as a third workspace-level view, gated by an extension to the `WorkspaceMode` enum.
- **Backend reuse:** Use existing `list_github_issues`, `add_github_label`, `remove_github_label`. No new Tauri commands unless strictly needed.
- **Per-user ordering:** Local only — the GitHub label model has no concept of within-column order, so ordering is stored in a local JSON file keyed by `(repo_path, column)`.
- **Virtualization:** Columns with >50 cards are virtualized.
- **No new heavy deps:** Prefer native HTML5 drag/drop over adding a library like `svelte-dnd-action`. If the native API is too limited for the acceptance criteria, reassess during implementation and surface the tradeoff.

## Architecture

### Workspace mode extension

Add `"kanban"` to `WorkspaceMode` in `stores.ts`. `App.svelte` renders `<KanbanBoard />` when `workspaceMode === "kanban"`, mirroring the existing `AgentDashboard` branch.

```ts
export type WorkspaceMode = "development" | "agents" | "kanban";
```

Hotkey: add a new action `{ type: "toggle-kanban-view" }` and wire an ambient-mode binding in `HotkeyManager.svelte` (proposed: `k`). Update `HotkeyHelp.svelte` and `WorkspaceModePicker.svelte` to expose the new mode.

### New components

- `src/lib/KanbanBoard.svelte` — top-level view. Loads issues for the focused project via `list_github_issues`, groups them into columns, renders the filter bar and columns.
- `src/lib/kanban/KanbanColumn.svelte` — single column. Handles drop targets, virtualization, empty state.
- `src/lib/kanban/KanbanCard.svelte` — single issue card. Renders title, `#number`, labels, assignee avatar, and is draggable.
- `src/lib/kanban/KanbanFilterBar.svelte` — filter chips for assignee / label / milestone.

### Pure logic modules (unit-tested)

- `src/lib/kanban/columns.ts` — column definitions, label ↔ column mapping, `columnForIssue(issue)` helper.
- `src/lib/kanban/filter.ts` — `applyFilters(issues, filters)`.
- `src/lib/kanban/ordering.ts` — `applyOrdering(issues, orderMap)`, move/reorder helpers. Per-user order stored in a local JSON file via two small Tauri commands `kanban_load_order` / `kanban_save_order` (file path: `<app-data>/kanban-order.json`).

Keeping the logic pure means filter, ordering, and column-mapping behaviour can be validated with Vitest unit tests that don't need a DOM or a running backend.

### Data model

`GithubIssue` in `stores.ts` already carries `labels: { name: string }[]`. Extend it as needed for assignee/milestone surfacing:

```ts
export interface GithubIssue {
  number: number;
  title: string;
  url: string;
  body?: string | null;
  labels: { name: string }[];
  assignees?: { login: string; avatarUrl?: string }[]; // new
  milestone?: { title: string } | null;                // new
}
```

`list_github_issues` backend command (in `commands/github.rs`) already shells out to `gh issue list --json ...`. Add `assignees` and `milestone` to the `--json` fields and the Rust struct. Parsing-only change, no new command.

### Column → label mapping

| Column       | Label                 | Empty-state copy                    |
| ------------ | --------------------- | ----------------------------------- |
| Backlog      | `status:backlog`      | "No backlog items"                  |
| To Do        | `status:todo`         | "Nothing queued"                    |
| In Progress  | `status:in-progress`* | "Nothing in progress"               |
| In Review    | `status:in-review`    | "Nothing awaiting review"           |
| Done         | `status:done`         | "Nothing done yet"                  |

*Only the `status:in-progress` label maps to "In Progress". The bare `in-progress` label used elsewhere in the app is unrelated and ignored by the board.

Issues with no status label default to **Backlog** in the view but are not auto-labelled — labelling happens on first drag.

### Drag-and-drop

Native HTML5 DnD:

1. `dragstart` on card writes `issue.number` to `dataTransfer`.
2. `dragover` on column sets `dropEffect = "move"` and highlights the column.
3. `drop` on column calls `moveIssue(issue, fromColumn, toColumn)`:
   - Optimistically update UI state.
   - Call `remove_github_label` for the old `status:*` label (if any).
   - Call `add_github_label` for the new `status:*` label.
   - On failure: revert UI state and surface a toast.

Ordering within a column is captured from the drop position (index in the column's card list) and persisted to the local order file keyed by `${repoPath}:${column}`.

### Filtering

`KanbanFilterBar` owns three selections: assignees (set), labels (set, excluding `status:*`), milestone (single). `applyFilters` intersects them; empty selection means "any". Filters apply across all columns simultaneously.

### Virtualization

For columns where `cards.length > 50`, render via a lightweight windowed list. Implement with a scroll container + computed visible slice (no external lib). If this turns out to be fiddly during implementation, switch to `@tanstack/svelte-virtual` and flag the dep addition.

### Persistence boundary

| Concern                   | Where it lives                                    |
| ------------------------- | ------------------------------------------------- |
| Column (status)           | GitHub label on the issue                         |
| Within-column order       | Local JSON: `<app-data>/kanban-order.json`        |
| Filter selection          | Component-local state; not persisted              |

## Validation

Per CLAUDE.md: for each unit below, reverting the implementation must make the test fail, and re-applying it must make it pass.

### Unit tests (Vitest)

- `columns.test.ts` — `columnForIssue` mapping across all five columns and the unlabelled-defaults-to-Backlog case.
- `filter.test.ts` — intersection of assignee/label/milestone selections, empty-selection semantics.
- `ordering.test.ts` — move-within-column, move-across-columns, load/save round-trip.

### Component tests (Vitest + Testing Library)

- `KanbanCard.test.ts` — renders title, number, labels, assignee; click opens issue URL.
- `KanbanColumn.test.ts` — empty state, >50 cards triggers virtualization branch.

### E2E test (Playwright, from the issue's acceptance criteria)

`e2e/kanban.spec.ts`:

1. Seed: create three issues via `create_github_issue`, one labelled `status:backlog`.
2. Open the kanban view (`k` hotkey).
3. Drag the seeded issue Backlog → To Do → In Progress → In Review → Done.
4. Reload the view.
5. Assert the issue is still in Done (status persisted via GitHub label).

E2E is skipped in CI without network / `gh` auth; runs locally via `pnpm test:e2e`.

## Out of scope (per issue)

WIP limits, swimlanes, multi-board views, real-time collaboration cursors — track as follow-ups if needed. Per-project configurable column sets also deferred to a follow-up.

## Open questions

- **Hotkey choice:** `k` for kanban is free per current `HotkeyHelp`. Confirm during implementation.
- **Assignee avatar URL:** `gh issue list --json assignees` returns login but not avatarUrl by default — may need a fallback to `https://github.com/<login>.png`.
- **Label color for new `status:*` labels:** align with Catppuccin palette already used elsewhere (e.g. `F9E2AF` for in-progress). Define once in `columns.ts`.
