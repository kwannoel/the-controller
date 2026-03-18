# Block Unauthenticated Cross-Origin Server API Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan task-by-task.

**Goal:** Prevent unauthenticated headless server deployments from accepting browser cross-origin API requests while preserving documented authenticated server access.

**Architecture:** Keep the existing auth middleware model, but replace the global permissive CORS behavior with a policy derived from whether `CONTROLLER_AUTH_TOKEN` is configured. In no-auth mode, do not emit browser CORS allow headers for API routes; in auth mode, continue allowing the existing browser client flow and keep static assets unaffected.

**Tech Stack:** Rust, Axum, tower-http CORS, cargo test, repo verification gates

---

### Task 1: Capture the security regression

**Files:**
- Modify: `src-tauri/src/bin/server.rs`
- Test: `src-tauri/src/bin/server.rs`

**Step 1: Write the failing test**

Add a unit/integration-style server test that builds the router in unauthenticated mode, sends an `OPTIONS` preflight and/or cross-origin `POST` to an `/api/*` route with an `Origin` header, and asserts the response does not grant CORS access.

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test --features server unauthenticated_api_requests_do_not_receive_cors_headers`

Expected: FAIL because the current router uses `CorsLayer::permissive()` and returns permissive browser CORS headers.

**Step 3: Write minimal implementation**

Refactor router construction so the CORS layer is created from configuration:
- authenticated mode: keep a permissive-enough API/browser policy
- unauthenticated mode: do not grant browser CORS access to API routes

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test --features server unauthenticated_api_requests_do_not_receive_cors_headers`

Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/bin/server.rs
git commit
```

### Task 2: Update operator-facing docs

**Files:**
- Modify: `README.md`

**Step 1: Write the failing test**

Not applicable; documentation reflects the tested behavior change.

**Step 2: Write minimal implementation**

Update the server-mode auth description so it no longer implies that unauthenticated mode is browser-accessible from arbitrary origins.

**Step 3: Verify docs**

Read the updated README section and confirm it matches the implemented server behavior.

### Task 3: Verify and finish branch

**Files:**
- Modify: `README.md`
- Modify: `src-tauri/src/bin/server.rs`

**Step 1: Run targeted verification**

Run:
- `cd src-tauri && cargo test --features server unauthenticated_api_requests_do_not_receive_cors_headers`
- `cd src-tauri && cargo test --features server authenticated_startup_message_does_not_print_raw_token`

**Step 2: Run repo gates**

Run:
- `pnpm check`
- `cd src-tauri && cargo fmt --check`
- `cd src-tauri && cargo clippy --features server -- -D warnings`

**Step 3: Review and finish**

Inspect the diff for correctness, then create a Conventional Commit with `closes #31` and the required trailer:

```text
Contributed-by: auto-worker
```
