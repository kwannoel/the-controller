# WebSocket Auth Token Decoding Design

## Definition

Issue `#15` is a server-mode auth mismatch. The browser adapter percent-encodes the `token` query parameter for `/ws`, but the server compares the raw query substring against `CONTROLLER_AUTH_TOKEN`. Tokens containing reserved characters therefore work for HTTP bearer auth and fail for WebSocket auth. The goal is to make WebSocket query auth accept the same token value after standard URL encoding.

## Constraints

- Keep the fix narrow to server-mode auth parsing in [`src-tauri/src/bin/server.rs`](/Users/silver/.the-controller/worktrees/the-controller/session-1-bf0d21/src-tauri/src/bin/server.rs).
- Preserve existing behavior for header auth and unauthed same-origin fallback when `CONTROLLER_AUTH_TOKEN` is unset.
- Decode only the `token` query parameter value before comparison; do not relax auth checks beyond standard percent-decoding.
- Add regression coverage with a token containing reserved characters so the previous implementation would fail.

## Approach

1. Add a server integration test that runs the auth middleware with `CONTROLLER_AUTH_TOKEN` set to a token containing reserved characters.
2. Verify an authenticated API request still succeeds and that a WebSocket handshake using a percent-encoded `?token=` value also succeeds and receives an event.
3. Update auth parsing to extract the `token` parameter from the query string, percent-decode it, and compare the decoded value to the configured token.

## Validation

- `cd src-tauri && cargo test --features server websocket_auth_accepts_percent_encoded_token_query`
- Revert the decoding logic and confirm the same test fails.
- Run repo-required gates after implementation: `pnpm check`, `cd src-tauri && cargo fmt --check`, `cd src-tauri && cargo clippy --features server -- -D warnings`, plus targeted frontend/Rust tests.
