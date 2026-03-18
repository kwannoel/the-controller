# Server Auth Token Stdout Design

## Definition

Issue `#19` is a credential leak in server mode. When `CONTROLLER_AUTH_TOKEN` is set, the server startup path prints a full `http://...?...token=...` URL to stdout, which is then persisted by the user-level systemd service into the journal. The goal is to stop emitting the raw token to stdout while keeping startup feedback useful for operators.

## Constraints

- Do not weaken existing auth behavior for `/api/*` and `/ws`.
- Keep startup logging informative enough for local operators to know where the server is listening.
- Follow TDD: add a regression test that fails before the fix and passes after it.
- Prefer a minimal change in `src-tauri/src/bin/server.rs`; harden any other obvious stdout leak only if it does not expand scope materially.
- Preserve existing repo conventions and run all required validation gates before merge.

## Approach

1. Extract the startup announcement into a small helper in `src-tauri/src/bin/server.rs`.
2. Add a unit test that proves authenticated mode never includes the raw token in the stdout-facing message.
3. Change the startup path to print a sanitized operator message without the secret and keep structured logs redacted.
4. Remove the deploy script's final token echo so the same credential is not exposed during deploy completion output.

## Validation

- Unit test in `src-tauri/src/bin/server.rs` fails against the old implementation because the raw token appears in the startup message.
- The same test passes after the fix.
- `pnpm check`
- `cd src-tauri && cargo fmt --check`
- `cd src-tauri && cargo clippy --features server --bin server -- -D warnings`
- Focused Rust test run for the new regression coverage, plus the repo-required gates above.
