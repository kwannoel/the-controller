# Server Auth Token Stdout Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan task-by-task.

**Goal:** Stop printing the server auth token to stdout while preserving usable startup messaging and regression coverage.

**Architecture:** Keep the change local to the server binary by routing startup messaging through a helper that can be unit tested. In authenticated mode, stdout should only mention the listening address and where to find the token, never the token value itself. The deploy script should stop echoing the same token during completion output.

**Tech Stack:** Rust, Axum server binary, Bash deploy script, cargo tests

---

### Task 1: Add the failing regression test

**Files:**
- Modify: `src-tauri/src/bin/server.rs`
- Test: `src-tauri/src/bin/server.rs`

**Step 1: Write the failing test**

Add a unit test around a startup-message helper so authenticated mode asserts:
- the raw token string is absent
- the address remains present
- the message tells the operator to read the token from config instead of stdout

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test --features server --bin server authenticated_startup_message_does_not_print_raw_token`

Expected: FAIL because the current startup path still embeds the raw token in the message.

### Task 2: Implement the minimal fix

**Files:**
- Modify: `src-tauri/src/bin/server.rs`

**Step 1: Add a startup-message helper**

Return:
- a stdout-safe message for operators
- a redacted structured-log message for tracing

**Step 2: Update startup logging**

Replace the inline `println!` and `tracing::info!` token formatting with the helper output.

**Step 3: Run the focused test**

Run: `cd src-tauri && cargo test --features server --bin server authenticated_startup_message_does_not_print_raw_token`

Expected: PASS

### Task 3: Remove the deploy script token echo

**Files:**
- Modify: `deploy/deploy.sh`

**Step 1: Update the completion output**

Print the URL and config path, but do not echo `CONTROLLER_AUTH_TOKEN`.

**Step 2: Keep operator guidance intact**

Tell operators to read the token from `$ENV_FILE` instead of printing it.

### Task 4: Verify, review, and ship

**Files:**
- Modify: `docs/plans/2026-03-17-server-auth-token-stdout-design.md`
- Modify: `docs/plans/2026-03-17-server-auth-token-stdout.md`
- Modify: `src-tauri/src/bin/server.rs`
- Modify: `deploy/deploy.sh`

**Step 1: Run repo validation**

Run:
- `cd src-tauri && cargo test --features server --bin server authenticated_startup_message_does_not_print_raw_token`
- `pnpm check`
- `cd src-tauri && cargo fmt --check`
- `cd src-tauri && cargo clippy --features server --bin server -- -D warnings`

**Step 2: Review the diff**

Confirm no raw auth token is printed to stdout or logs by the changed paths.

**Step 3: Commit**

Use a conventional commit and include:
- `closes #19`
- `Contributed-by: auto-worker`
