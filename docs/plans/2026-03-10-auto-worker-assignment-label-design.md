# Auto-Worker Assignment Label Design

## Definition

Add a persistent `assigned-to-auto-worker` GitHub label so worker-owned issues carry durable provenance without overloading the generic `in-progress` label. The dashboard should treat a worker issue as completed when it has `assigned-to-auto-worker` and the issue is closed.

## Constraints

- Keep `in-progress` as the generic "currently being worked on in a session" label.
- Add `assigned-to-auto-worker` when the auto-worker claims an issue.
- Remove `in-progress` whenever worker ownership ends.
- Keep `assigned-to-auto-worker` only if the issue ended up closed.
- Remove `assigned-to-auto-worker` if the worker stops and the issue is still open.
- Eliminate dependence on `finished-by-worker` for normal reporting.
- Use TDD for behavior changes in both backend label handling and frontend report loading.
- Historical worker issues should be backfilled with a one-off migration rather than complex runtime inference from git history.

## Approaches

### 1. Add `assigned-to-auto-worker` and derive completion from issue state

Keep `in-progress` for active ownership, add `assigned-to-auto-worker` for worker provenance, and consider closed worker-labeled issues completed.

Pros: Smallest long-term state model, avoids fragile terminal cleanup bookkeeping, matches the user's preferred semantics.
Cons: A closed worker-owned issue is treated as completed even if closure happened by a path other than a merged worker PR.

### 2. Keep `finished-by-worker` and add `assigned-to-auto-worker`

Preserve the current finished label, but also add a provenance label when work starts.

Pros: Most explicit state, distinguishes ownership from successful completion.
Cons: Still relies on cleanup paths correctly maintaining a second terminal label, which is exactly what failed here.

### 3. Infer worker provenance from git history and report comments

Avoid new labels and reconstruct worker history from commit trailers, report comments, and PR linkage.

Pros: Minimal label footprint.
Cons: Runtime inference is brittle, expensive, and harder to explain than explicit issue metadata.

## Chosen Design

Use approach 1.

When the scheduler claims an eligible issue, it should add both `in-progress` and `assigned-to-auto-worker`. When the worker exits or is cleaned up during restart recovery, it should always remove `in-progress`. If the issue is closed at that time, keep `assigned-to-auto-worker`. If the issue is still open, remove `assigned-to-auto-worker` because the worker did not complete it.

The dashboard should stop querying `finished-by-worker` and instead query closed issues labeled `assigned-to-auto-worker`. The auto-worker pane must continue to show these completed issues as its report list. Opening an item should show the latest `<!-- auto-worker-report -->` comment when present. If a historical or migrated issue has no worker report comment, it should still appear in the pane with a fallback body such as `No worker report was posted for this issue.` Worker report comments remain useful detail when present, but they are no longer the sole index for completed work. For historical issues, run a one-off migration that adds `assigned-to-auto-worker` to issues already containing the `<!-- auto-worker-report -->` marker, then optionally remove any stale `finished-by-worker` labels.

## Validation

- Add backend tests proving issue claim adds `assigned-to-auto-worker`.
- Add backend tests proving successful worker cleanup removes `in-progress` and retains `assigned-to-auto-worker` when the issue is closed.
- Add backend tests proving unsuccessful worker cleanup removes both `in-progress` and `assigned-to-auto-worker` when the issue remains open.
- Add command-level tests proving worker report queries load closed issues with `assigned-to-auto-worker`.
- Add command-level tests proving issues without worker report comments still appear in the auto-worker pane with fallback detail text.
- Run targeted Rust and frontend tests in a failing state first, then rerun after implementation.
- Run a one-off migration against the repository and verify previously missing worker issues appear in the dashboard.
