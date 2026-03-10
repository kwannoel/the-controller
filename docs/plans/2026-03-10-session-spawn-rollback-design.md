# Session Metadata Rollback On PTY Spawn Failure Design

## Summary

`create_session` and `unarchive_session` currently persist project/session metadata before `PtyManager::spawn_session` runs. If spawn fails, storage keeps a session that was never actually launched, which leaves archived state flipped or a brand-new session entry orphaned on disk.

## Goals

- Make session persistence transactional around PTY spawn for the affected command paths.
- Preserve the current success path and lock ordering expectations.
- Keep the fix narrow to metadata consistency; do not broaden it into unrelated worktree cleanup or session lifecycle changes.

## Approach Options

### Option 1: Spawn first, persist after success

Build the session config, attempt spawn, then save project metadata only when spawn succeeds.

Pros:
- Avoids rollback logic.
- Leaves storage untouched on failure.

Cons:
- Changes the current ordering guarantee that metadata exists before the live session starts.
- Creates a new failure mode where a running session can exist without persisted metadata if the final save fails.

### Option 2: Persist first, then roll back on spawn failure

Save the intended project mutation, attempt the PTY spawn, and restore the prior project state if the spawn step returns an error.

Pros:
- Preserves the current success-path ordering.
- Matches the issue remediation guidance directly.
- Can be shared by both `create_session` and `unarchive_session`.

Cons:
- Requires explicit rollback handling and tests for the failure path.

## Recommended Design

Use Option 2, but implement rollback as a compensating mutation against the latest reloaded project state instead of writing an older full-project snapshot back to disk. Extract a small helper in `commands.rs` that:

1. Loads the current project.
2. Applies a caller-provided mutation.
3. Saves the mutated project.
4. Runs a caller-provided post-save action.
5. If the action fails, reloads the latest project, applies a caller-provided rollback mutation for just the affected session change, and saves again.

`create_session` uses the helper to append the new `SessionConfig` and remove just that session entry on failure. `unarchive_session` uses it to flip the existing session’s `archived` flag to `false` and flip it back if spawn fails. For the create path, also clean up the newly created worktree/branch when spawn fails so rollback does not leave hidden worktree state behind.

## Error Handling

- Return the original PTY spawn failure when the compensating rollback succeeds.
- If rollback save fails, return an error that includes both the spawn failure and rollback failure so the user knows storage may need manual repair.
- For `create_session`, if worktree cleanup fails after the metadata rollback, return an error that includes both the spawn failure and the cleanup failure.
- Keep the helper scoped to one project file update so unrelated concurrent project changes are preserved.

## Testing

- Add a regression test for the transactional helper that saves a new session, simulates a post-save failure, and proves the session entry is removed.
- Add a regression test for the same helper in the unarchive case, proving the archived flag is restored when the post-save action fails.
- Add a regression test showing concurrent unrelated project changes are preserved when the rollback runs.
- Add a focused test for the create-session cleanup path that removes the created worktree/branch.
- Verify the tests fail without the rollback helper and pass with it.
