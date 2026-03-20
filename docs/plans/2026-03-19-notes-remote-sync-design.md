# Notes Remote Sync — Design

## Overview

Push notes to a remote GitHub repository after each local commit. The remote is auto-detected: if the notes git repo at `~/.the-controller/notes/` has an `origin` remote configured, push to it. Otherwise, do nothing.

User sets up the remote once:
```bash
cd ~/.the-controller/notes
git remote add origin git@github.com:user/my-notes.git
```

## Architecture

### `push_to_remote(base: &Path) -> Result<(), String>`

New function in `src-tauri/src/notes.rs`:

1. Opens the notes repo at `{base}/notes/`
2. Checks if an `origin` remote exists — if not, returns `Ok(())` silently
3. Shells out to `git push origin HEAD` in the notes directory
   - Uses `std::process::Command`, not git2, to inherit the user's SSH agent and credential helpers
4. Returns `Err(stderr)` on non-zero exit

### `commit_and_sync(base: &Path, message: &str, emitter: ...)`

New function that combines commit + push:

1. Calls `commit_notes(base, message)`
2. If a commit was created, spawns `std::thread::spawn` to call `push_to_remote(base)`
3. On push failure, emits `notes-sync-error` event with the error message

### Command layer

Update `commands::commit_notes` to call `commit_and_sync` instead of `notes::commit_notes` directly, passing the emitter from `AppState`.

### Frontend

Listen for `notes-sync-error` event and show a toast notification.

## Decisions

- **Shell out for push**: git2's push requires credential callback plumbing for SSH/HTTPS. Shelling out to `git` inherits the user's SSH agent and credential helpers for free.
- **Push-only**: Local is the source of truth. No pull/merge to avoid conflict complexity. Two-way sync can be added later.
- **Event-driven**: Push after each commit rather than on a timer. Simpler, no new scheduler.
- **Auto-detect remote**: No UI for remote configuration. User manages `origin` via CLI.

## Testing

- `push_to_remote` returns `Ok(())` when no remote is configured
- `push_to_remote` succeeds when remote exists (local bare repo as remote in test)
- `commit_and_sync` emits `notes-sync-error` when push fails
