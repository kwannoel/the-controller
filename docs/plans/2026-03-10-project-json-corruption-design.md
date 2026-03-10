# Surface Corrupt Project JSON Design

## Summary

`Storage::list_projects` currently reads every `project.json` under `~/.the-controller/projects/`, keeps only entries that deserialize into `Project`, and silently discards malformed files. That makes corrupted metadata look identical to a deleted project. The fix should preserve valid projects while returning explicit corruption diagnostics so callers can surface the problem.

## Goals

- Keep valid projects available even when one `project.json` is corrupt.
- Return structured corruption details instead of silently skipping malformed files.
- Surface the warning in the UI path used by project listing.
- Preserve existing scheduler and restore behavior by letting internal callers continue operating on valid projects.

## Non-Goals

- Building a new diagnostics panel.
- Attempting automatic repair of corrupt files.
- Blocking all project operations because one metadata file is malformed.

## Approach Options

### Option 1: Fail `list_projects` on the first corrupt file

Pros:

- Simple backend change.
- Makes corruption impossible to ignore.

Cons:

- One bad `project.json` would take down the full project list, scheduler startup, duplicate-name checks, and session restore.
- Worse user experience than the current bug.

### Option 2: Log corruption to stderr and keep returning `Vec<Project>`

Pros:

- Minimal API churn.
- Preserves all current callers.

Cons:

- Still invisible in the app UI.
- Callers cannot test or react to corruption details.

### Option 3: Return valid projects plus structured corrupt-entry diagnostics

Pros:

- Keeps working projects available.
- Gives both backend callers and the UI explicit data about corruption.
- Supports regression tests at the storage and UI layers.

Cons:

- Requires updating list command/result types and a few consumers.

Recommendation: Option 3.

## Design

Introduce a storage result type:

- `ProjectInventory { projects: Vec<Project>, corrupt_entries: Vec<CorruptProjectEntry> }`
- `CorruptProjectEntry` includes the project directory path, `project.json` path, and parse error text.

`Storage::list_projects` will:

1. Read every project directory.
2. Deserialize valid `project.json` files into `Project`.
3. Record malformed files in `corrupt_entries` instead of dropping them silently.
4. Still return I/O errors for directory traversal or file reads that genuinely prevent listing.

Tauri commands:

- `list_projects` and `list_archived_projects` will return inventory objects rather than raw arrays.
- Internal Rust callers that only need valid projects will use the inventory’s `projects` field and emit warnings to stderr when `corrupt_entries` is non-empty.

Frontend:

- `Sidebar.svelte` will accept the richer list command result.
- Valid projects still populate the store.
- The UI will show an error toast summarizing new corrupt entries so users can tell why a project disappeared.
- The toast logic should avoid spamming identical warnings on every refresh by tracking already-surfaced corruption keys in component state.

## Testing

- Rust: add a storage regression test with one valid and one malformed `project.json`, asserting both the retained valid project and the reported corrupt entry.
- Frontend: add a `Sidebar` test that mocks `list_projects` returning a corrupt entry and asserts `showToast` is called with a corruption warning.
