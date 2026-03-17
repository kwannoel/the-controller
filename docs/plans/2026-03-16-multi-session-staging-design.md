# Multi-Session Staging Design

## Problem

Staging is limited to one session at a time per project. The `staged_session` field is singular (`Option<StagedSession>`), the socket path is hardcoded, and the hotkey assumes a single staged session. Users should be able to stage multiple sessions simultaneously.

## Design

### Data model

Change `staged_session: Option<StagedSession>` to `staged_sessions: Vec<StagedSession>` on the `Project` model (Rust) and `staged_sessions: StagedSession[]` on the frontend TypeScript interface.

Migration: when loading a project, if the old `staged_session` field is present, convert it to a single-element `staged_sessions` vec and drop the old field.

### Socket path

Replace the hardcoded `/tmp/the-controller-staged.sock` with per-session sockets: `/tmp/the-controller-staged-{session_id}.sock`.

- `staged_socket_path()` becomes `staged_socket_path(session_id: &Uuid) -> String`
- Each staged instance gets `CONTROLLER_SOCKET` set to its own socket path
- The main instance's socket listener handles all staged sockets

### stage_session_core changes

- Check whether *this specific session* is already in `staged_sessions` (not whether any session is staged)
- On success, push the new `StagedSession` into the vec
- Socket path passed to the spawned process uses the session ID

### unstage_session changes

- Accept a `session_id` parameter (currently only takes `project_id`)
- Remove only the matching entry from `staged_sessions`
- Kill only that entry's process group
- Delete only that entry's socket file

### Hotkey behavior

Toggle staging for the sidebar-focused session:
- If the focused session is staged, unstage it
- If the focused session is not staged, stage it

### Frontend

- `project.staged_session` → `project.staged_sessions` throughout
- ProjectTree badge: check `project.staged_sessions.some(s => s.session_id === session.id)`
- Sidebar stageSession/unstageSession: pass session_id to unstage

### Staging lock

Keep the global `staging_lock` mutex. It serializes staging operations, which is still correct — multiple stagings can't race the port-finding logic.

## Files to modify

- `src-tauri/src/models.rs` — StagedSession field on Project
- `src-tauri/src/commands.rs` — stage_session_core, unstage_session, stage_session
- `src-tauri/src/status_socket.rs` — staged_socket_path signature
- `src-tauri/src/lib.rs` — any cleanup referencing staged_socket_path
- `src/lib/stores.ts` — Project interface
- `src/lib/sidebar/ProjectTree.svelte` — staged badge check
- `src/lib/Sidebar.svelte` — stageSession/unstageSession functions
- `src/lib/HotkeyManager.svelte` — hotkey toggle logic
