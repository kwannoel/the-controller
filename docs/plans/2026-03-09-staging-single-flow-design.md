# Staging Single-Flow Design

## Problem

Pressing `v` (stage) with uncommitted changes only sends a commit prompt to Claude and bails — requiring a second `v` press. Same for rebase conflicts. Should be a single action: commit → rebase → stage.

## Design

Mirror the `merge_session_branch` pattern already in the codebase.

### Backend Changes (`src-tauri/src/commands.rs`)

**Make `stage_session_inplace` async with progress events:**

- `pub fn` → `pub async fn`, add `app_handle: AppHandle` parameter
- Use `tokio::task::spawn_blocking` for git operations (per domain-knowledge.md)
- Emit `"staging-status"` events at each phase

**Flow:**

1. **Commit phase**: If worktree is dirty, send commit prompt to Claude via PTY, emit `"staging-status": "Waiting for commit..."`, poll `is_worktree_clean` every 3s. Timeout after 60s.

2. **Rebase phase**: Sync main, check if behind. If rebase hits conflicts, send resolve prompt to Claude, emit `"staging-status": "Rebase conflicts (attempt N/5). Claude is resolving..."`, poll `is_rebase_in_progress` every 3s. Retry up to 5 attempts (same as merge).

3. **Stage phase**: Proceed with `stage_inplace` as before.

**Constants:** Reuse `REBASE_POLL_INTERVAL_SECS` (3s). Add `COMMIT_POLL_INTERVAL_SECS` (3s), `MAX_COMMIT_WAIT_SECS` (60).

### Frontend Changes (`src/lib/Sidebar.svelte`)

**`stageSessionInplace` function:**

- Focus terminal and active session before invoking (like `mergeSession`)
- Listen for `"staging-status"` events, show toasts
- Clean up listener in finally block

### Lock Safety

Same pattern as existing code: acquire `pty_manager` lock briefly to write prompt, release before await. Acquire `storage` lock only at the end for the actual staging mutation.
