# Kanban Board Implementation Plan (Issue #537)

Single bundled PR. See `2026-04-22-kanban-board-design.md` for the design.

## Task structure (CLAUDE.md)

- **Definition:** add a full-screen kanban view that groups GitHub issues by `status:*` label, supports drag/drop to change status, filtering, per-user within-column ordering, virtualization, and an E2E test.
- **Constraints:** GitHub labels as the status source; reuse existing Tauri GitHub commands; native HTML5 DnD (no new DnD dep); per-user order persisted in local JSON via two new Tauri commands.
- **Validation:** revert-must-fail unit tests for `columns.ts` / `filter.ts` / `ordering.ts`; component tests for `KanbanCard` / `KanbanColumn`; one E2E test covering the issue's acceptance criterion (create → drag across all columns → reload → status persists).

## Step sequence

Each step ends in a state where `pnpm test` and `cargo test` pass.

### 1. Pure logic modules (TDD)

Create `src/lib/kanban/` with three modules plus their tests. No UI yet.

- `columns.ts`: `Column` type, `COLUMNS` ordered list, `LABEL_BY_COLUMN`, `columnForIssue(issue): Column` (unlabelled → `Backlog`).
- `filter.ts`: `Filters` type `{ assignees: Set<string>, labels: Set<string>, milestone: string | null }`, `applyFilters(issues, filters): GithubIssue[]`. `status:*` labels excluded from the `labels` set.
- `ordering.ts`: `OrderMap = Record<string, number[]>` (key `${repoPath}:${column}` → issue numbers), `applyOrdering(issues, column, repoPath, orderMap)`, `moveIssue(orderMap, fromKey, toKey, issueNumber, toIndex)`.

Write `columns.test.ts`, `filter.test.ts`, `ordering.test.ts` first and make them red, then implement.

### 2. Backend: extend `list_github_issues` + add order-file commands

In `src-tauri/src/commands/github.rs`:

- Extend `GithubIssue` struct with `assignees: Vec<Assignee>` and `milestone: Option<Milestone>`.
- Extend the `gh issue list --json` arg string to include `assignees,milestone`.
- `Assignee { login: String, avatar_url: Option<String> }` — `gh` returns `login` only by default; frontend falls back to `https://github.com/<login>.png`.

New file `src-tauri/src/commands/kanban.rs`:

- `kanban_load_order(app: AppHandle) -> Result<serde_json::Value, String>` — read `<app_data_dir>/kanban-order.json`, return `{}` if missing.
- `kanban_save_order(app: AppHandle, order: serde_json::Value) -> Result<(), String>` — atomic write (tmp + rename) to the same path.

Wire both in `lib.rs` `invoke_handler`.

Rust tests: `kanban::tests` round-trips save/load through a tempdir.

Mirror the new `GithubIssue` fields in `src/lib/stores.ts`.

### 3. Workspace mode + hotkey + empty board shell

- Add `"kanban"` to `WorkspaceMode` in `stores.ts`.
- Add `{ type: "toggle-kanban-view" }` to `HotkeyAction`.
- In `HotkeyManager.svelte`: bind `k` in ambient mode to toggle between `development` and `kanban` via `workspaceMode.update(m => m === "kanban" ? "development" : "kanban")`.
- Update `HotkeyHelp.svelte` (help table) and `WorkspaceModePicker.svelte` (picker entry).
- Create `src/lib/KanbanBoard.svelte` that only renders a titled empty shell for now.
- In `App.svelte`, extend the workspace-mode branch: `{:else if workspaceModeState.current === "kanban"} <KanbanBoard />`.

Component test: `KanbanBoard.test.ts` — renders 5 column headers in order.

### 4. Columns, cards, load data

- `src/lib/kanban/KanbanColumn.svelte`: props `{ column: Column, issues: GithubIssue[] }`, renders header + card list + empty-state copy.
- `src/lib/kanban/KanbanCard.svelte`: props `{ issue: GithubIssue }`, renders `#number`, title, non-`status:*` labels, assignee avatars (`avatarUrl ?? https://github.com/${login}.png`). Click → `openUrl(issue.url)`.
- `KanbanBoard.svelte` loads issues via `command<GithubIssue[]>("list_github_issues", { repoPath })` using the focused project's `repo_path`, groups via `columnForIssue`, renders five `KanbanColumn`s.
- Loading / error states parallel `IssuesModal`.

Component tests: `KanbanCard.test.ts`, `KanbanColumn.test.ts`.

### 5. Drag-and-drop to change status

- Cards: `draggable="true"`, `dragstart` sets `e.dataTransfer.setData("text/plain", String(issue.number))` and a module-level `draggingIssueRef` for the full object.
- Columns: `dragover` → `e.preventDefault(); dropEffect="move"` + `isDropTarget` flag; `dragleave` clears the flag; `drop` reads the issue number and calls `moveIssue`.
- `moveIssue(issue, fromColumn, toColumn)` in `KanbanBoard.svelte`:
  1. Optimistic local state update.
  2. `await command("remove_github_label", { repoPath, issueNumber, label: LABEL_BY_COLUMN[fromColumn] })` when the old label exists on the issue.
  3. `await command("add_github_label", { repoPath, issueNumber, label: LABEL_BY_COLUMN[toColumn], description, color })`.
  4. On rejection: revert local state, `showToast(String(e), "error")`.
- Persist updated `OrderMap` via `kanban_save_order` on every successful drop.

No new component tests for DnD — covered by E2E.

### 6. Filter bar

- `src/lib/kanban/KanbanFilterBar.svelte`: three controls (assignees multi-select, labels multi-select excluding `status:*`, milestone single-select), derived from the current issue set.
- `KanbanBoard.svelte`: state `filters: Filters`, pass filtered list via `applyFilters` to each column.

No new tests — `filter.ts` is already unit-tested and the wiring is thin.

### 7. Per-user ordering

- `KanbanBoard.svelte` on mount: `orderMap = await command("kanban_load_order")`.
- Render issues per column via `applyOrdering(issuesForColumn, column, repoPath, orderMap)`.
- On drop, compute the target index from the drop Y position relative to the column's card elements, call `moveIssue` in `ordering.ts`, then `kanban_save_order`.

### 8. Virtualization

- Inside `KanbanColumn.svelte`: if `issues.length > 50`, render via a windowed list. Implementation: fixed card height constant, compute `startIndex`/`endIndex` from `scrollTop`, render a padded spacer above and below.
- Component test: asserts that only ~visible slice is rendered when `issues.length === 200`.

### 9. E2E test

`e2e/kanban.spec.ts`:

1. Launch app (reuse existing Playwright harness in `e2e/`).
2. Seed an issue via `create_github_issue` on a test repo (or a test double — match the convention used in existing e2e specs).
3. Press `k` to open the board.
4. Drag the seeded card Backlog → To Do → In Progress → In Review → Done, asserting its column between each drag.
5. Reload, press `k`, assert it's still in Done.

If the existing e2e harness doesn't support `gh`-backed state, gate this spec behind the same env flag other gh-dependent e2e specs use.

### 10. Polish

- Catppuccin palette on column headers (match existing tones in `app.css`).
- Keyboard affordance on cards: `Enter` opens the issue URL, even without a keyboard-DnD menu.
- `HotkeyHelp` entry for `k`.

## Risk / fallback checkpoints

- **Native HTML5 DnD feels wrong** (e.g. can't scroll the column while dragging): switch to `svelte-dnd-action`, add to `package.json`, flag in the PR.
- **Virtualization math gets fiddly**: swap to `@tanstack/svelte-virtual`, same — flag the dep addition.
- **`gh issue list --json assignees,milestone` returns unexpected shape**: adjust Rust struct; run one manual `gh` call against `kwannoel/the-controller` to confirm the JSON shape before wiring the frontend.

## Out of scope (to reiterate)

WIP limits, swimlanes, multi-board views, real-time collaboration cursors, per-project configurable column sets. All follow-ups.

## PR

Title: `feat: kanban board for tracking issues`

Body must contain `closes #537`.
