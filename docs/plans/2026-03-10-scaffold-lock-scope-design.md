# Scaffold Project Lock Scope Design

## Summary

`scaffold_project` currently takes `state.storage.lock()` before duplicate-name checks and keeps that mutex guard alive through directory creation, git initialization, template writes, the initial commit, and the external `gh repo create` and `git push` subprocesses. That turns a project-local scaffold into a global storage stall and also violates the repo rule that slow Tauri commands must run off the main thread.

## Goals

- Minimize the storage lock scope so only duplicate detection and final persistence use it.
- Keep the existing scaffold behavior and rollback semantics intact.
- Move the slow scaffold work off the Tauri main thread.
- Add a regression test that proves unrelated storage access is not blocked for the duration of a stalled scaffold.

## Constraints

- Preserve the current duplicate-name rule: reject persisted non-archived projects with the same name.
- Preserve the current success path: create local repo, write template files, publish to GitHub, then save the project entry.
- Preserve rollback behavior on GitHub creation or initial push failures.
- Avoid broad storage or scheduler refactors; the issue is lock scope inside `scaffold_project`.

## Approach Options

### Option 1: Keep `scaffold_project` synchronous and only shrink the lock window

Read the config and duplicate state under a short-lived storage lock, drop it, then continue with the existing synchronous scaffold logic.

Pros:
- Small code diff for the mutex issue itself.
- Keeps the command signature unchanged.

Cons:
- Still blocks the Tauri main thread during filesystem and subprocess work.
- Leaves a known repo rule violation in place.

### Option 2: Make `scaffold_project` async and run scaffold work in `spawn_blocking`

Read the minimal storage state up front, move the long-running scaffold flow into a blocking worker, then reacquire the lock only to persist the final `Project`.

Pros:
- Fixes the storage contention and the main-thread blocking problem together.
- Matches the established command pattern in this repo for slow work.
- Keeps the mutex scope explicit and easy to audit.

Cons:
- Slightly larger signature change because the command becomes `async`.

## Recommended Design

Use Option 2.

Split `scaffold_project` into three phases:

1. Acquire `state.storage` briefly to load the config, list projects, and reject duplicates or pre-existing directories.
2. Run the scaffold side effects in `tokio::task::spawn_blocking`, including repo creation, template writes, git commit, GitHub publish, and rollback on failure. This phase returns the constructed `Project` without touching storage.
3. Reacquire `state.storage` after the blocking task completes, re-check duplicate-name uniqueness while holding the mutex, and persist the finished `Project` only if the name is still available. If another command claimed the name while scaffolding was in flight, roll back the scaffolded local and remote repo instead of saving duplicate metadata.

This keeps the global storage mutex unavailable only for short metadata reads and the final save, while all filesystem and external process work happens without the lock and off the UI thread.

## Error Handling

- Keep the existing rollback helpers and failure messages.
- Propagate `spawn_blocking` join failures as `Task failed: ...` to match other commands.
- Re-check directory existence before scaffolding starts from the data captured in phase 1; no additional locking is needed because the persisted duplicate-name check remains authoritative for app state.

## Testing

- Add a command-level regression test that starts `scaffold_project` on a separate thread with a fake `gh repo create` script that blocks on a sentinel file.
- While that fake `gh` command is sleeping, attempt to acquire `app_state.storage.lock()` from the test thread and assert it succeeds immediately.
- Release the sentinel so scaffolding completes, then assert the project is saved successfully.
- Add a concurrency regression test that blocks `scaffold_project`, claims the same name through `create_project`, then verifies `scaffold_project` rolls its local and remote state back instead of saving a duplicate project name.
- Run the existing scaffold rollback tests to ensure the refactor did not change failure cleanup behavior.
