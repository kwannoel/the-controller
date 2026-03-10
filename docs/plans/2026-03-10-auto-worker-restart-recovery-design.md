# Auto-Worker Restart Recovery Design

## Problem

The auto-worker scheduler assumes every app startup begins with zero active worker sessions. In development that is false: tmux-backed sessions intentionally survive restarts, and multiple app instances can overlap briefly. The current startup logic removes every `in-progress` label and starts a fresh scheduler loop with an empty in-memory `active_sessions` map. That lets duplicate workers spawn for the same project and issue, while completed worker sessions can remain persisted indefinitely.

## Goal

Make auto-worker startup restart-safe so one project resumes at most one live auto-worker session, completed sessions are cleaned up, and stale `in-progress` labels are only removed when no live worker still owns them.

## Constraints

- Tmux session persistence in dev is intentional and must remain intact.
- Auto-worker still guarantees one active worker per project.
- Startup cleanup must not kill or detach a genuinely active worker that is still processing an issue.
- The existing scheduler architecture is in-memory plus persisted project session records; the fix should stay minimal and local to that model.

## Options

### 1. Rebuild startup state from persisted worker sessions

- On scheduler start, scan enabled projects for persisted `auto_worker_session` entries.
- For each worker session:
  - if the tmux session still exists, treat it as an active restored worker;
  - if the tmux session is gone, clean its persisted session and stale labels.
- Skip removing `in-progress` on issues still owned by restored live workers.

Pros: directly fixes the demonstrated restart bug with limited surface area.
Cons: does not prevent two completely separate app instances from each restoring the same worker unless restoration is deduplicated by persisted state.

### 2. Add a cross-process scheduler lock

- Use a filesystem lock so only one auto-worker scheduler thread runs globally.

Pros: stronger process-level protection.
Cons: more invasive, still needs startup restoration to preserve live tmux workers after restart.

### 3. Persist scheduler state separately

- Store active worker ownership in dedicated durable state instead of reconstructing it from sessions.

Pros: more explicit long-term model.
Cons: more code and migration cost than this bug requires.

## Decision

Use option 1 now.

The minimal viable fix is to reconstruct startup state from persisted auto-worker sessions and tmux liveness, then only clean labels/sessions that are truly orphaned. That addresses the observed duplicate spawning without redesigning the whole scheduler.

## Design

### Startup restoration

Add a restoration step before the poll loop starts work:

- Read enabled, non-archived projects from storage.
- For each persisted `auto_worker_session`:
  - if its tmux session exists, restore it into the scheduler’s in-memory `active_sessions` map;
  - if its tmux session does not exist, remove the stale persisted session, clean its worktree if needed, and remove `in-progress` from its issue.

If multiple persisted worker sessions exist for one project:

- keep the newest live session as the project’s active worker;
- treat the others as stale duplicates and clean them up.

### Label cleanup

Replace the unconditional startup `in-progress` sweep with restoration-aware cleanup:

- labels for issues owned by restored live workers stay intact;
- labels for stale persisted sessions are removed during stale-session cleanup;
- labels with no corresponding live or persisted worker are still removed as orphaned.

### Completion behavior

The existing exit path stays mostly the same. Once a restored worker exits, the normal completion branch can mark the issue finished, remove labels, and remove the persisted session.

## Testing

Add a focused unit test for the startup restoration policy:

- create persisted worker sessions representing one live worker and one stale worker;
- assert startup restoration keeps the live one active and marks only the stale one for cleanup.

This test must fail on current behavior because startup currently restores nothing and treats all labels as stale.
