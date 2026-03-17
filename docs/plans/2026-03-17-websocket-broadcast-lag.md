# WebSocket Broadcast Lag Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan task-by-task.

**Goal:** Keep server-mode WebSocket clients connected when their broadcast receiver reports lag instead of a terminal close.

**Architecture:** Exercise the real `/ws` boundary with a small broadcast channel, then make the forwarding loop explicitly distinguish lag from closure. The fix remains local to `src-tauri/src/bin/server.rs` and uses TDD to prove the regression.

**Tech Stack:** Rust, Tokio broadcast channels, Axum WebSockets, tokio-tungstenite for the test client

---

### Task 1: Add the failing lag regression test

**Files:**
- Modify: `src-tauri/src/bin/server.rs`
- Modify: `src-tauri/Cargo.toml`

**Step 1: Write the failing test**

Add a server test that:
- starts a router with `.route("/ws", get(ws_upgrade))`
- seeds `ServerState` with a `broadcast::channel(2)`
- connects with a WebSocket client
- sends enough events to trigger `RecvError::Lagged(_)`
- verifies a later event is still received on the same socket

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test --features server websocket_client_recovers_after_broadcast_lag`

Expected: FAIL because the socket closes after the receiver lags.

### Task 2: Implement the minimal fix

**Files:**
- Modify: `src-tauri/src/bin/server.rs`

**Step 1: Update the forwarding loop**

Handle `broadcast::error::RecvError::Lagged(_)` explicitly with `continue`, and break only on `Closed` or WebSocket send failure.

**Step 2: Run the regression test**

Run: `cd src-tauri && cargo test --features server websocket_client_recovers_after_broadcast_lag`

Expected: PASS.

### Task 3: Verify and finalize

**Files:**
- Modify: `src-tauri/src/bin/server.rs`
- Modify: `src-tauri/Cargo.toml`
- Create: `docs/plans/2026-03-17-websocket-broadcast-lag-design.md`
- Create: `docs/plans/2026-03-17-websocket-broadcast-lag.md`

**Step 1: Run repo verification gates**

Run:
- `pnpm check`
- `cd src-tauri && cargo fmt --check`
- `cd src-tauri && cargo clippy --features server --bin server --tests -- -D warnings`

**Step 2: Commit**

Run:

```bash
git add docs/plans/2026-03-17-websocket-broadcast-lag-design.md \
        docs/plans/2026-03-17-websocket-broadcast-lag.md \
        src-tauri/Cargo.toml \
        src-tauri/src/bin/server.rs
git commit
```

Use summary: `fix: keep ws clients alive after broadcast lag`

Body must include:
- `closes #36`
- `Contributed-by: auto-worker`
