# WebSocket Broadcast Lag Design

## Definition

Fix the server-mode WebSocket event stream so a client that falls behind the broadcast backlog is not disconnected permanently. Today `handle_ws` exits on any `broadcast::Receiver::recv()` error, which incorrectly treats `RecvError::Lagged(_)` as terminal closure.

## Constraints

- Keep the fix scoped to server-mode WebSocket forwarding in `src-tauri/src/bin/server.rs`.
- Preserve the existing `/ws` contract, authentication flow, and reconnect behavior for real closures.
- Follow repo workflow: test first, verify the regression fails before the fix, then run the required verification commands.
- Prefer a regression test at the network boundary rather than a brittle loop-only unit test.

## Approach

1. Add a WebSocket integration test in `src-tauri/src/bin/server.rs` that:
   - serves `/ws` using `ws_upgrade`
   - uses a small broadcast channel to force a lagged receiver
   - confirms the socket still receives a later event after lag occurs
2. Update `handle_ws` to continue on `broadcast::error::RecvError::Lagged(_)` and only break on:
   - `Closed`
   - WebSocket send failure
3. Keep logging/testing focused on the regression only.

## Validation

- The new regression test must fail against the original `while let Ok(msg)` implementation because the socket closes after lag.
- After the fix, the same test must pass and show a later message arriving on the same socket.
- Run:
  - `cargo test --features server websocket_client_recovers_after_broadcast_lag`
  - `pnpm check`
  - `cd src-tauri && cargo fmt --check`
  - `cd src-tauri && cargo clippy --features server --bin server --tests -- -D warnings`
