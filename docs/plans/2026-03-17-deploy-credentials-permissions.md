# Deploy Credentials Permissions Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan task-by-task.

**Goal:** Ensure deploy credentials are persisted with restrictive owner-only file permissions on Unix.

**Architecture:** Keep the change inside `src-tauri/src/deploy/credentials.rs`. Add a Unix-only regression test around file permissions first, then replace the write path with an explicit open-and-write flow that sets `0600` at creation time and re-applies it after updates.

**Tech Stack:** Rust, serde, tempfile, Unix file permissions

---

### Task 1: Add the Unix regression test

**Files:**
- Modify: `src-tauri/src/deploy/credentials.rs`
- Test: `src-tauri/src/deploy/credentials.rs`

**Step 1: Write the failing test**

Add a Unix-only test that:
- sets `HOME` to a temp directory
- saves `DeployCredentials`
- asserts `~/.the-controller/deploy-credentials.json` exists with mode `0o600`
- broadens the mode to `0o644`
- saves again
- asserts the mode is back to `0o600`

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test deploy::credentials::tests::save_sets_owner_only_permissions_on_unix`
Expected: FAIL because the current implementation creates the file through `std::fs::write`, which does not specify restrictive create-time permissions.

**Step 3: Write minimal implementation**

Replace the save path with an explicit file open/write helper that sets mode `0o600` on Unix and preserves `0o600` after updates.

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test deploy::credentials::tests::save_sets_owner_only_permissions_on_unix`
Expected: PASS

**Step 5: Commit**

```bash
git add docs/plans/2026-03-17-deploy-credentials-permissions-design.md docs/plans/2026-03-17-deploy-credentials-permissions.md src-tauri/src/deploy/credentials.rs
git commit -m "fix: harden deploy credential file permissions"
```

### Task 2: Run repository verification gates

**Files:**
- Verify only: `src-tauri/src/deploy/credentials.rs`

**Step 1: Run frontend gate**

Run: `pnpm check`
Expected: PASS

**Step 2: Run Rust formatting gate**

Run: `cd src-tauri && cargo fmt --check`
Expected: PASS

**Step 3: Run Rust lint gate**

Run: `cd src-tauri && cargo clippy -- -D warnings`
Expected: PASS

**Step 4: Run Rust tests**

Run: `cd src-tauri && cargo test`
Expected: PASS
