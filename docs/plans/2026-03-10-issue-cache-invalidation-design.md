# Issue Cache Invalidation for Auto-Worker Label Edits Design

## Problem

`AppState` keeps GitHub issue lists in `IssueCache` for up to 60 seconds, and the UI command layer updates that cache when users add or remove labels through Tauri commands. The auto-worker does not use those commands. It shells out to `gh issue edit` through private synchronous helpers in `src-tauri/src/auto_worker.rs`, so successful label changes leave the cached repo entry untouched until the TTL expires.

The result is stale issue state in `IssuePickerModal` and the assigned issue panels after the auto-worker claims work, finishes work, or cleans stale labels.

## Goal

Ensure any successful auto-worker label mutation makes the next GitHub issue read for that repo refetch fresh data instead of serving stale cached issues.

## Constraints

- Keep the fix local to the existing cache model; do not redesign GitHub issue state management.
- Preserve the current UI command behavior, which still performs precise in-memory cache mutations for direct user actions.
- Cover the bug with a regression test that fails before the implementation and passes after it.
- Avoid broad refactors of the auto-worker scheduler or GitHub command layer.

## Options

### 1. Invalidate the repo cache entry after successful auto-worker label edits

- Add an `IssueCache` invalidation method for a repo path.
- Thread the cache handle into the auto-worker label-edit path.
- After a successful `gh issue edit`, remove that repo entry from the cache.

Pros: minimal, correct for all auto-worker label edits, and aligns with the issue report.
Cons: the next issue fetch pays for a full refetch instead of a targeted in-memory patch.

### 2. Teach the auto-worker helpers to mirror direct cache mutations

- Pass issue number and label data into `IssueCache::add_label` and `IssueCache::remove_label`.
- Keep the cache hot without forcing a refetch.

Pros: avoids an extra GitHub fetch on the next read.
Cons: duplicates mutation logic across two code paths and is easier to get wrong when auto-worker operations become more complex.

### 3. Route auto-worker label edits through the Tauri GitHub command functions

- Consolidate label editing into one shared async/cache-aware path.

Pros: one source of truth long term.
Cons: disproportionately invasive for a narrow bug, because the auto-worker currently relies on synchronous helper functions in background scheduler code.

## Decision

Use option 1.

The auto-worker only needs cache correctness, not cache precision. Invalidating the repo entry after successful label edits is the smallest change that fixes all current stale-label paths, including startup cleanup, issue pickup, issue completion, and label migration.

## Design

### Cache API

Add `IssueCache::invalidate(&mut self, repo_path: &str)` in `src-tauri/src/state.rs`. It should remove the repo entry entirely.

### Auto-worker label helper

Refactor the synchronous label-edit helper flow in `src-tauri/src/auto_worker.rs` so the command execution and cache invalidation are separated cleanly:

- a small helper performs the `gh issue edit`;
- a wrapper invalidates the repo cache only when that edit succeeds.

Thread `&AppState` into the auto-worker call sites that already own state, so all successful auto-worker label edits invalidate the cache.

### Failure behavior

If `gh issue edit` fails, return the existing error and leave the cache untouched. A failed mutation should not evict otherwise-valid cached data.

## Testing

Add a regression test in `src-tauri/src/auto_worker.rs` around the new wrapper helper:

- seed `IssueCache` with a repo entry;
- simulate a successful label edit via an injected closure and assert the repo entry is invalidated;
- simulate a failed label edit and assert the cache entry remains present.

This test should fail before the new invalidation path exists and pass once the helper uses `IssueCache::invalidate`.
