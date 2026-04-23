# Web Frontend Parity — Design

**Goal:** Bring the web frontend (Svelte served by `vite` + axum server binary at `src-tauri/src/bin/server.rs`) to feature parity with the Tauri desktop frontend so the desktop target can be deprecated. Motivation: the web frontend is playwright-testable end-to-end; the Tauri target is not.

## Current state

- **Shared code path:** `src/` is one Svelte app. `src/lib/backend.ts` already dispatches `command(cmd, args)` to either Tauri `invoke` or `fetch("/api/" + cmd)`, and `listen(event, handler)` to either Tauri events or the shared `/ws` WebSocket. All business events pass through `WsBroadcastEmitter`, so event parity is already achieved for app logic.
- **Backend gap:** 59 unique `#[tauri::command]` handlers in `src-tauri/src/commands.rs`. The axum server exposes 12 routes (`src-tauri/src/bin/server.rs:33-46`); `create_session` is stubbed (501). **~45 commands have no HTTP equivalent.**
- **Frontend leaks:** 7 files import `@tauri-apps/*` directly, bypassing `backend.ts`. They fail silently in web mode:
  - `src/main.ts:1` — `invoke("log_frontend_error")`
  - `src/App.svelte:5` — `@tauri-apps/api/window`
  - `src/lib/Terminal.svelte:7`, `AgentDashboard.svelte:5`, `IssuesModal.svelte:4`, `kanban/KanbanCard.svelte:2` — `@tauri-apps/plugin-opener`
  - `src/lib/clipboard.ts:1` — `@tauri-apps/plugin-clipboard-manager`
  - `src/lib/finish-branch.ts` — takes `invoke` as a parameter; callers pass Tauri's `invoke` directly
- **Test harness:** `playwright.config.ts:10-22` already boots axum + vite. The chat daemon (`~/projects/the-controller-daemon`) is **not** booted, so chat e2e tests would fail.

## Open scope questions (resolve before Pass 2)

1. **Tauri deletion shape.** Delete `src-tauri/src/lib.rs`, `tauri.conf.json`, every `#[tauri::command]` wrapper, and ship `src-tauri/src/bin/server.rs` as the sole backend — or keep Tauri buildable during the transition and just converge the frontend on `backend.ts`? Decision affects whether handlers get *moved* into `server.rs` or *duplicated* during the migration.
2. **Platform-only features.** For `copy_image_file_to_clipboard`, `capture_app_screenshot`, and OS drag-drop with file paths — drop entirely, or reimplement with browser APIs (File System Access, `html2canvas`, DataTransfer)?

## Inventory of missing HTTP routes

Grouped by domain. Check = has HTTP route in server.rs today.

### Covered (13)

`list_projects`, `check_onboarding`, `restore_sessions`, `connect_session`, `load_project`, `write_to_pty`, `send_raw_to_pty`, `resize_pty`, `close_session`, `list_archived_projects`, `merge_session_branch`, plus `/ws`. `create_session` is stubbed.

### Pass 1 — unblock web mode golden path

| Command | Why P0 |
|---|---|
| `create_session` (un-stub) | Can't create sessions in web |
| `read_daemon_token` | Chat workspace mode bootstrap is blocked |
| `log_frontend_error` | `main.ts` error reporting currently hits a direct `invoke` |

Also in Pass 1 — frontend cleanup:

- Route `@tauri-apps/plugin-opener` through a new `backend.openUrl(url)` that falls back to `window.open(url, "_blank", "noopener")`. Update the 4 `*.svelte` call sites.
- Replace the direct `invoke("log_frontend_error")` in `main.ts` with `backend.command(...)`.
- Refactor `finish-branch.ts` to use `backend.command` instead of receiving `invoke` as a parameter.
- Gate `getCurrentWindow()` in `App.svelte` behind the `isTauri` check (or move the logic to a no-op in web).
- `src/lib/clipboard.ts` image-paste path: detect Tauri via `isTauri` and fall back to `navigator.clipboard.read()` in web; acceptable degradation for P1 is "text-only paste in web".

**Exit criterion:** `pnpm dev` + `cargo run --bin server --features server` yields a browser at `http://localhost:1420` where you can open an existing project, spawn a session, and see terminal output.

### Pass 2 — feature parity

| Domain | Commands |
|---|---|
| Project lifecycle | `create_project`, `delete_project`, `list_project_prompts`, `save_onboarding_config` |
| Session detail | `get_session_commits`, `get_session_token_usage`, `get_repo_head` |
| Staging | `stage_session`, `unstage_session` |
| Prompts / agents.md | `save_session_prompt`, `set_initial_prompt`, `get_agents_md`, `update_agents_md` |
| GitHub | `list_github_issues`, `create_github_issue`, `close_github_issue`, `delete_github_issue`, `post_github_comment`, `add_github_label`, `remove_github_label`, `generate_issue_body` |
| Kanban | `kanban_load_order`, `kanban_save_order` |
| Secure env | `submit_secure_env_value`, `cancel_secure_env_request` |
| Onboarding / filesystem | `check_claude_cli`, `start_claude_login`, `stop_claude_login`, `scaffold_project`, `generate_project_names`, `list_directories_at`, `list_root_directories`, `home_dir` |

Most of these are thin wrappers — the logic already lives in `the_controller_lib`. The server handler only needs arg unpacking + error mapping. Plan to add a small macro or helper to cut boilerplate.

### Pass 3 — background features + harness

- Maintainer + auto-worker HTTP wiring (12 commands): `trigger_maintainer_check`, `get_maintainer_status`, `get_maintainer_issues`, `get_maintainer_issue_detail`, `get_maintainer_history`, `clear_maintainer_reports`, `configure_maintainer`, `get_auto_worker_queue`, `get_worker_reports`, `configure_auto_worker`, `list_assigned_issues`.
- Add the chat daemon to `playwright.config.ts` `webServer` list (or a script that spawns it before tests).
- Platform-only features (`copy_image_file_to_clipboard`, `capture_app_screenshot`) — decide per Q2 above.

## Deprecation steps (after Pass 3)

Deferred until the three passes land and the web target has been exercised by a full playwright run:

1. Delete `src-tauri/src/lib.rs` and the `#[tauri::command]` wrappers in `commands.rs` (keep the underlying functions — they're shared library code).
2. Remove `tauri.conf.json`, `src-tauri/capabilities/`, `src-tauri/icons/`, `src-tauri/build.rs`.
3. Drop `@tauri-apps/*` dependencies from `package.json`; remove `src/lib/backend.ts`'s Tauri branch (`fetch`/`WebSocket` only).
4. Rename `src-tauri/` → `server/` (or similar) and update build scripts.
5. Update `README.md`, `ARCHITECTURE.md`, `CLAUDE.md`.

## Validation

- **Pass 1:** playwright smoke test — boot web stack, open a seeded project, spawn session, write to PTY, observe output.
- **Pass 2:** extend existing e2e specs in `e2e/specs/` to run under the `e2e` project (currently chromium against `localhost:1420`). Each command needs a test that fails if the route is removed.
- **Pass 3:** a chat-mode e2e spec that sends a message and observes agent output.

At each pass, per `CLAUDE.md` task structure: Definition / Constraints / Validation must be stated before code.
