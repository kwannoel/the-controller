# Project JSON Corruption Design

## Context

`Storage::list_projects` currently reads every `project.json` under `~/.the-controller/projects` and silently drops entries that fail to deserialize. That makes corrupted metadata disappear from the UI and lets backend flows behave as though the project never existed.

## Goals

- Keep listing healthy projects even when one `project.json` is corrupt.
- Surface corruption details to the UI so the user can act on them.
- Prevent duplicate-name and duplicate-repo checks from proceeding when project metadata is corrupt.
- Keep background restore/scheduler flows operating on valid projects while logging explicit corruption details.

## Options Considered

### 1. Fail the entire project listing on the first corrupt file

This is simple, but it would hide all valid projects and make the app feel broken because one bad metadata file blocks the full list.

### 2. Keep silently filtering bad files and only log to stderr

This preserves current behavior and does not solve the user-facing problem. It also leaves duplicate checks unsafe.

### 3. Return a project scan result with valid projects plus corruption diagnostics

This keeps healthy projects visible, lets the UI display concrete warnings, and gives command/scheduler code enough information to decide when corruption must block an operation.

## Decision

Use option 3.

`Storage` will scan project metadata into a result object with:

- `projects`: successfully deserialized `Project` entries
- `corrupt_entries`: file/path/error details for malformed `project.json` files

Frontend project-list commands will return that richer payload, and the sidebar/app state will render a persistent corruption warning instead of repeatedly spamming transient errors.

Backend behavior:

- Project listing and archived listing return valid projects plus diagnostics.
- Duplicate-name / duplicate-repo checks fail loudly if corruption exists because they cannot safely prove uniqueness.
- Session restore and scheduler code continue using valid projects, but they log corruption details instead of silently ignoring them.

## Error Presentation

The sidebar will show a warning banner when corrupt metadata exists. Each entry will include the affected file path and the parse error so the user can find and fix or remove the broken file.

## Testing

- Rust storage test: one valid `project.json` plus one corrupt `project.json` returns one project and one corruption diagnostic.
- Rust command test: create/load flows fail with an explicit corruption message when duplicate checks cannot trust the metadata set.
- Svelte test: sidebar loads a project scan response and renders the corruption warning.
