# Server API Parity Design

## Definition

Expose the deploy and keybinding commands in server mode so the browser frontend can call the same backend surface as the desktop Tauri app. The issue exists because `src-tauri/src/lib.rs` registers these commands for desktop IPC while `src-tauri/src/bin/server.rs` does not expose matching `/api/*` routes.

## Constraints

- Preserve the existing browser command contract from `src/lib/backend.ts`, which posts JSON to `/api/<command>`.
- Reuse the existing Rust command behavior instead of re-implementing deploy or keybinding logic in the server binary.
- Keep server-mode parity maintainable so future desktop commands do not silently drift away from the Axum route table.
- Follow repo workflow: test-first, then implementation, then full verification (`pnpm check`, `cargo fmt --check`, `cargo clippy -- -D warnings`).

## Approach

1. Add a server regression test that parses the desktop invoke list and server route table from source and compares them.
2. Allow only explicitly intentional server-only routes in the parity test.
3. Add the missing Axum routes and handlers for:
   - `detect_project_type`
   - `get_deploy_credentials`
   - `save_deploy_credentials`
   - `is_deploy_provisioned`
   - `deploy_project`
   - `list_deployed_services`
   - `load_keybindings`
4. Implement handlers as thin wrappers around the existing library commands.

## Validation

- The new parity test must fail before the route additions and pass after them.
- A targeted Cargo test run must cover the parity regression.
- Repo verification gates must pass after the change.
