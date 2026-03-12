# Fix Staging Race Condition

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Prevent concurrent `stage_session` calls from spawning duplicate staging instances that collide on the same port, causing white screen of death.

**Architecture:** Add an in-memory `Mutex<()>` guard (`staging_lock`) to `AppState` that serializes the entire `stage_session` operation. Fix `find_staging_port` to check both IPv4 and IPv6 binds. Fix zombie leak by reaping the child instead of `mem::forget`.

**Tech Stack:** Rust (Tauri v2), `std::sync::Mutex`, `libc`, `std::net::TcpListener`

---

## Root Cause Summary

Three bugs found:

1. **Race condition (primary):** `stage_session` is `async` and only holds the storage `Mutex` briefly at the start (orphan check) and end (save PID). Two concurrent calls both pass the orphan check before either saves, spawning two `bash ./dev.sh` on the same port. The second call's PID overwrites the first's record, making the first staging instance untrackable.

2. **IPv4-only port check:** `find_staging_port` binds `127.0.0.1` (IPv4) but Vite listens on `[::1]` (IPv6). A port occupied on IPv6 is invisible to the check.

3. **Zombie leak:** `std::mem::forget(child)` prevents Rust from ever reaping the child. When the child dies, it becomes a zombie. The comment claims the opposite.

---

### Task 1: Add `staging_lock` to prevent concurrent staging

**Files:**
- Modify: `src-tauri/src/state.rs:81-104`
- Test: `src-tauri/src/commands.rs` (existing `staging_tests` module near line 3330)

**Step 1: Write the failing test**

Add to the `staging_tests` module in `src-tauri/src/commands.rs`:

```rust
#[test]
fn test_find_staging_port_checks_ipv6() {
    // Bind on IPv6 only — find_staging_port must detect this
    let listener = std::net::TcpListener::bind("[::1]:0").unwrap();
    let occupied_port = listener.local_addr().unwrap().port();
    let base = occupied_port.checked_sub(STAGING_PORT_OFFSET).unwrap();
    let port = find_staging_port(base).unwrap();
    assert_ne!(port, occupied_port, "must skip port occupied on IPv6");
}
```

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test staging_tests::test_find_staging_port_checks_ipv6 -- --nocapture`
Expected: FAIL — `find_staging_port` returns the IPv6-occupied port because it only checks IPv4.

**Step 3: Add `staging_lock` field to `AppState`**

In `src-tauri/src/state.rs`, add a `tokio::sync::Mutex<()>` to `AppState`:

```rust
use tokio::sync::Mutex as TokioMutex;
```

Add field to the `AppState` struct (line ~86):

```rust
pub staging_lock: TokioMutex<()>,
```

Initialize it in `from_storage` (line ~92):

```rust
staging_lock: TokioMutex::new(()),
```

**Why `tokio::sync::Mutex` instead of `std::sync::Mutex`?** The `stage_session` function is `async` and `.await`s across the lock. A `std::sync::Mutex` cannot be held across `.await` points (it's not `Send`). A `tokio::sync::Mutex` can.

**Step 4: Acquire `staging_lock` at the top of `stage_session`**

In `src-tauri/src/commands.rs`, at the very start of `stage_session` (line ~859, after the UUID parsing):

```rust
let _staging_guard = state.staging_lock.lock().await;
```

This serializes the entire function — no two staging calls can interleave. The guard drops when the function returns (success or error), releasing the lock.

**Step 5: Fix `find_staging_port` to check both IPv4 and IPv6**

Replace the body of `find_staging_port` (lines 812-826):

```rust
fn find_staging_port(base_port: u16) -> Result<u16, String> {
    let start = base_port
        .checked_add(STAGING_PORT_OFFSET)
        .ok_or("Port overflow")?;
    for candidate in start..start.saturating_add(100) {
        let ipv4_free = std::net::TcpListener::bind(("127.0.0.1", candidate)).is_ok();
        let ipv6_free = std::net::TcpListener::bind(("::1", candidate)).is_ok();
        if ipv4_free && ipv6_free {
            return Ok(candidate);
        }
    }
    Err(format!(
        "No free port found in range {}-{}",
        start,
        start + 100
    ))
}
```

**Step 6: Run test to verify it passes**

Run: `cd src-tauri && cargo test staging_tests -- --nocapture`
Expected: ALL pass, including the new `test_find_staging_port_checks_ipv6`.

**Step 7: Update existing port test for IPv6 awareness**

The existing `test_find_free_port_skips_occupied` binds `127.0.0.1:0`. Add a companion that binds `[::1]:0` to verify both are skipped. The new test from step 1 already covers this — verify both tests pass.

**Step 8: Commit**

```bash
git add src-tauri/src/state.rs src-tauri/src/commands.rs
git commit -m "fix: prevent concurrent staging with lock + check IPv6 ports"
```

---

### Task 2: Fix zombie leak from `std::mem::forget(child)`

**Files:**
- Modify: `src-tauri/src/commands.rs:1058-1071`

**Step 1: Understand the problem**

`std::mem::forget(child)` on a `std::process::Child` means Rust never calls `wait()` on the child. When the child dies, the kernel keeps a zombie entry because the parent hasn't reaped it. The comment on line 1070 is incorrect.

The correct approach: spawn a background thread that calls `child.wait()` to reap the zombie when it exits. We still track the process via PID/process group for killing.

**Step 2: Replace `mem::forget` with background reaper**

Replace lines 1068-1071:

```rust
let pid = child.id();
// Deliberately leak the child handle — we manage the process via PID/process group,
// not via the Child handle. This avoids zombie entries from an unwaited child.
std::mem::forget(child);
```

With:

```rust
let pid = child.id();
// Reap the child in a background thread to prevent zombie entries.
// We manage the process lifetime via PID/process group (kill_process_group),
// not via this Child handle.
std::thread::spawn(move || {
    let _ = child.wait();
});
```

**Step 3: Run all staging tests**

Run: `cd src-tauri && cargo test staging_tests -- --nocapture`
Expected: PASS (no behavioral change to existing tests; this fixes a resource leak only).

**Step 4: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "fix: reap staging child process to prevent zombie"
```

---

### Task 3: Add frontend debounce guard

**Files:**
- Modify: `src/lib/Sidebar.svelte:418-438`

**Step 1: Understand the problem**

Even with the backend lock, rapid double-clicks on "Stage" fire two Tauri commands. The first acquires the lock and runs; the second blocks until the first finishes, then runs a second staging attempt. The backend lock serializes them but doesn't reject the duplicate. Adding a frontend guard prevents the second call entirely.

**Step 2: Add `staging` state flag**

Near the top of the `<script>` block in `Sidebar.svelte`, add:

```typescript
let staging = false;
```

**Step 3: Guard `stageSession` with the flag**

Wrap the function body:

```typescript
async function stageSession(projectId: string, sessionId: string) {
    if (staging) return;
    staging = true;
    activeSessionId.set(sessionId);
    focusTerminalSoon();

    const unlistenStatus = listen<string>("staging-status", (payload) => {
      showToast(payload, "info");
    });

    try {
      await command("stage_session", { projectId, sessionId });
      await loadProjects();
      const session = projectList
        .find((p) => p.id === projectId)
        ?.sessions.find((s) => s.id === sessionId);
      showToast(`Staged ${session?.label ?? "session"} — launching on separate port`, "info");
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      staging = false;
      unlistenStatus?.();
    }
  }
```

**Step 4: Run frontend tests**

Run: `npx vitest run`
Expected: PASS

**Step 5: Commit**

```bash
git add src/lib/Sidebar.svelte
git commit -m "fix: debounce stage button to prevent double-click race"
```

---

## Verification

After all tasks are done:

1. Build: `cd src-tauri && cargo test`
2. Frontend: `npx vitest run`
3. Manual test: launch app, stage a session, verify no zombie processes (`ps aux | grep defunct`), verify no duplicate staging processes
