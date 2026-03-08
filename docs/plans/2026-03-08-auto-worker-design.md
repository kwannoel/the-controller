# Auto-Worker: Background Coding Agent

## Problem

Simple, high-priority issues (typos, small bug fixes, config changes) don't need human-in-the-loop orchestration. Currently users must manually pick issues and spawn background workers with `C`/`X`. We want an auto-pilot mode that continuously works through eligible issues.

## Design

### Behavior

A per-project auto-worker that continuously processes issues labeled `priority: high` + `complexity: low`:

1. When enabled, immediately check for eligible issues (priority: high, complexity: low, triaged, not in-progress).
2. Spawn one session at a time per project — full Claude Code PTY in a worktree, with background worker prompt.
3. When session finishes (idle/exit), check for next eligible issue immediately.
4. When no issues remain, poll every 5 minutes for newly-triaged ones.
5. On completion, add `finished-by-worker` label to the issue.

### Stuck Prevention (3 layers)

1. **Strong system prompt** — "Never ask questions. Never wait for input. Make your best judgment and proceed."
2. **Auto-nudge** — If session goes idle (Claude waiting for input), automatically send "Continue working autonomously. Do not ask questions or wait for input." After 3 nudges, kill the session.
3. **Hard timeout** — 30 min max per session. Kill and move on.

### UI

- **`o` prefix mode** — `o` then `m` toggles maintainer, `o` then `w` toggles auto-worker. Replaces the current single `o` toggle.
- **`b` panel** — Maintainer panel gains an "Auto-worker" section showing: enabled/disabled, current issue (if working), completed count, failed count.
- **Sessions are hidden** from sidebar — they're internal to the auto-worker. No terminal view.

### Issue Eligibility

An issue is eligible if it has ALL of:
- `priority: high` label
- `complexity: low` label
- `triaged` label

And NONE of:
- `in-progress` label
- `finished-by-worker` label

### Session Lifecycle

```
[enabled] → check for eligible issues
  ├── found → add `in-progress` label → spawn session → monitor
  │     ├── session exits normally → add `finished-by-worker` → check next
  │     ├── session idle (waiting for input) → auto-nudge (up to 3x) → kill
  │     └── session exceeds 30 min → kill → check next
  └── none found → sleep 5 min → check again
```

### Architecture

#### Rust Backend

**New module: `auto_worker.rs`**
- `AutoWorkerScheduler::start(app_handle)` — background thread, similar to `MaintainerScheduler`
- Tracks per-project state: enabled, current session ID, nudge count, spawn time
- Uses existing `create_session` infrastructure (worktree + PTY + hooks)
- Listens to session status events to detect idle/exit
- Emits `auto-worker-status:{project_id}` events for frontend

**Model additions:**
- `AutoWorkerConfig` on `Project` (enabled, analogous to `MaintainerConfig`)
- `AutoWorkerStatus` enum: `Idle`, `Working { issue_number, session_id }`, `Disabled`

**New commands:**
- `configure_auto_worker` — enable/disable per project
- `get_auto_worker_status` — current status for panel display

#### Frontend

**HotkeyManager changes:**
- `o` enters "toggle mode" (similar to jump mode with `g`)
- `m` in toggle mode → toggle maintainer
- `w` in toggle mode → toggle auto-worker
- Any other key or timeout → cancel toggle mode

**MaintainerPanel changes:**
- Add "Auto-worker" section below maintainer section
- Show: enabled/disabled toggle, current issue, stats (completed/failed)

**commands.ts changes:**
- Replace `toggle-maintainer` with `toggle-mode` prefix
- Add `toggle-maintainer` and `toggle-auto-worker` as chord targets

### Not in Scope (v1)

- Configurable concurrency (always 1 per project)
- Configurable timeout (always 30 min)
- Viewing the hidden session's terminal output
- Auto-worker for Codex (Claude only for now)
