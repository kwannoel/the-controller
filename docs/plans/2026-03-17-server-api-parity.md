# Server API Parity Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan task-by-task.

**Goal:** Expose the missing deploy and keybinding server-mode API routes and add a parity regression test so browser mode stays aligned with the desktop command surface.

**Architecture:** The Axum server remains a thin transport layer over the shared library commands. A source-based regression test compares the Tauri invoke registration in `src-tauri/src/lib.rs` with the server route table in `src-tauri/src/bin/server.rs`, with a narrow allowlist for intentional server-only routes.

**Tech Stack:** Rust, Tauri v2 command library, Axum, serde_json, Cargo tests

---

### Task 1: Add the failing parity regression test

**Files:**
- Modify: `src-tauri/src/bin/server.rs`
- Test: `src-tauri/src/bin/server.rs`

**Step 1: Write the failing test**

Add a test that:
- reads `src/lib.rs`
- extracts `commands::...` and `deploy::commands::...` names from `tauri::generate_handler![...]`
- reads `src/bin/server.rs`
- extracts `/api/<command>` route names
- asserts that server routes include the desktop commands, except for a tiny explicit allowlist of server-only routes

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test --features server server_routes_cover_desktop_command_surface`

Expected: FAIL with the current missing deploy/keybinding route names in the diff

**Step 3: Commit**

Do not commit yet. Continue to Task 2 after the red state is confirmed.

### Task 2: Implement the missing server handlers and routes

**Files:**
- Modify: `src-tauri/src/bin/server.rs`
- Reference: `src-tauri/src/deploy/commands.rs`
- Reference: `src-tauri/src/commands.rs`

**Step 1: Write minimal implementation**

Add Axum routes and wrapper handlers for:
- `detect_project_type`
- `get_deploy_credentials`
- `save_deploy_credentials`
- `is_deploy_provisioned`
- `deploy_project`
- `list_deployed_services`
- `load_keybindings`

Each handler should deserialize the expected JSON payload shape and delegate to the existing library command function or helper.

**Step 2: Run targeted test to verify it passes**

Run: `cd src-tauri && cargo test --features server server_routes_cover_desktop_command_surface`

Expected: PASS

### Task 3: Full verification and review

**Files:**
- Modify: `src-tauri/src/bin/server.rs`
- Create: `docs/plans/2026-03-17-server-api-parity-design.md`
- Create: `docs/plans/2026-03-17-server-api-parity.md`

**Step 1: Run repo verification**

Run:
- `pnpm check`
- `cd src-tauri && cargo fmt --check`
- `cd src-tauri && cargo clippy --features server -- -D warnings`

**Step 2: Self-review**

Inspect the diff for:
- exact route parity
- no duplicated deploy logic
- no extra behavior changes

**Step 3: Commit**

Use a Conventional Commit message that includes `closes #14` and the required trailer:

```text
fix: expose server deploy and keybinding routes

closes #14

Contributed-by: auto-worker
```
