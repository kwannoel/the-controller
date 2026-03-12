# Staging via Separate Controller Instance — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Replace in-place staging (git checkout in main repo) with launching a separate Controller dev instance from the session's worktree.

**Architecture:** When user presses `v`, the backend ensures the worktree is clean (commit/rebase phases unchanged), runs `npm install` if needed, picks a free port, and spawns `dev.sh <port>` from the worktree as a child process group. The PID and port are stored in `StagedSession`. Pressing `v` again kills the process group and clears the state. The main Controller's title bar shows which session is staged.

**Tech Stack:** Rust (Tauri commands, `std::process::Command`, `nix` for process group kill), Svelte 5 (stores, hotkey handlers), tmux (shared sessions)

---

### Task 1: Update `StagedSession` model (Rust)

**Files:**
- Modify: `src-tauri/src/models.rs:25-30`

**Step 1: Write the failing test**

Add a test in `src-tauri/src/models.rs` at the bottom of the `mod tests` block:

```rust
#[test]
fn test_staged_session_new_format_roundtrip() {
    let project = Project {
        id: Uuid::new_v4(),
        name: "test".to_string(),
        repo_path: "/tmp".to_string(),
        created_at: "2026-03-11T00:00:00Z".to_string(),
        archived: false,
        maintainer: MaintainerConfig::default(),
        auto_worker: AutoWorkerConfig::default(),
        prompts: vec![],
        sessions: vec![],
        staged_session: Some(StagedSession {
            session_id: Uuid::new_v4(),
            pid: 12345,
            port: 2420,
        }),
    };
    let json = serde_json::to_string(&project).expect("serialize");
    let deserialized: Project = serde_json::from_str(&json).expect("deserialize");
    let staged = deserialized.staged_session.unwrap();
    assert_eq!(staged.pid, 12345);
    assert_eq!(staged.port, 2420);
}
```

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test test_staged_session_new_format_roundtrip`
Expected: FAIL — `StagedSession` still has `original_branch`/`staging_branch` fields

**Step 3: Update the `StagedSession` struct**

In `src-tauri/src/models.rs`, replace lines 23-30:

```rust
/// Tracks staging state: which session is running as a separate
/// Controller instance, and the PID/port of that process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedSession {
    pub session_id: Uuid,
    pub pid: u32,
    pub port: u16,
}
```

Also update the doc comment on the `staged_session` field in `Project` (line 18-19):

```rust
    /// When a session is staged as a separate Controller instance.
    #[serde(default)]
    pub staged_session: Option<StagedSession>,
```

**Step 4: Fix the old `test_staged_session_roundtrip` test**

Update the existing test at line ~657 to use the new fields:

```rust
#[test]
fn test_staged_session_roundtrip() {
    let project = Project {
        id: Uuid::new_v4(),
        name: "test".to_string(),
        repo_path: "/tmp".to_string(),
        created_at: "2026-03-09T00:00:00Z".to_string(),
        archived: false,
        maintainer: MaintainerConfig::default(),
        auto_worker: AutoWorkerConfig::default(),
        prompts: vec![],
        sessions: vec![],
        staged_session: Some(StagedSession {
            session_id: Uuid::new_v4(),
            pid: 99999,
            port: 2420,
        }),
    };
    let json = serde_json::to_string(&project).expect("serialize");
    let deserialized: Project = serde_json::from_str(&json).expect("deserialize");
    let staged = deserialized.staged_session.unwrap();
    assert_eq!(staged.pid, 99999);
    assert_eq!(staged.port, 2420);
}
```

**Step 5: Run all model tests**

Run: `cd src-tauri && cargo test models::tests`
Expected: ALL PASS

**Step 6: Commit**

```bash
git add src-tauri/src/models.rs
git commit -m "refactor: update StagedSession model to track pid and port instead of branches"
```

---

### Task 2: Add port selection helper (Rust)

**Files:**
- Modify: `src-tauri/src/commands.rs` (add helper function near the staging constants)

**Step 1: Write the failing test**

Add a test in `src-tauri/src/commands.rs` (or a new test module at the bottom):

```rust
#[cfg(test)]
mod staging_tests {
    use super::*;

    #[test]
    fn test_find_free_port_returns_offset_port_when_free() {
        // Port 59123 is unlikely to be in use
        let port = find_staging_port(58123).unwrap();
        assert_eq!(port, 59123); // base + 1000
    }

    #[test]
    fn test_find_free_port_skips_occupied() {
        // Bind a port to make it occupied
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let occupied_port = listener.local_addr().unwrap().port();
        // Ask for a base where offset lands on occupied port
        let base = occupied_port - 1000;
        let port = find_staging_port(base).unwrap();
        // Should skip the occupied port and return next one
        assert!(port > occupied_port);
        assert!(port <= occupied_port + 100);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test staging_tests`
Expected: FAIL — `find_staging_port` doesn't exist

**Step 3: Implement the helper**

Add near the staging constants in `src-tauri/src/commands.rs`:

```rust
const STAGING_PORT_OFFSET: u16 = 1000;

/// Find a free port for the staged Controller instance.
/// Starts at base_port + 1000 and increments until a free port is found.
fn find_staging_port(base_port: u16) -> Result<u16, String> {
    let start = base_port.checked_add(STAGING_PORT_OFFSET)
        .ok_or("Port overflow")?;
    for candidate in start..start.saturating_add(100) {
        if std::net::TcpListener::bind(("127.0.0.1", candidate)).is_ok() {
            return Ok(candidate);
        }
    }
    Err(format!("No free port found in range {}-{}", start, start + 100))
}
```

**Step 4: Run tests**

Run: `cd src-tauri && cargo test staging_tests`
Expected: ALL PASS

**Step 5: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "feat: add find_staging_port helper for staged controller instances"
```

---

### Task 3: Implement `stage_session` command (Rust)

**Files:**
- Modify: `src-tauri/src/commands.rs` (replace `stage_session_inplace`)

**Step 1: Replace the `stage_session_inplace` command**

Replace the `stage_session_inplace` function (lines 809-984) and `unstage_session_inplace` (lines 986-1008) with new implementations. Keep the commit and rebase phases (phases 1 and 2) identical. Replace phase 3 (the git checkout) with process spawning.

```rust
#[tauri::command]
pub async fn stage_session(
    state: State<'_, AppState>,
    _app_handle: AppHandle,
    project_id: String,
    session_id: String,
) -> Result<(), String> {
    use crate::models::StagedSession;

    let project_uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let session_uuid = Uuid::parse_str(&session_id).map_err(|e| e.to_string())?;

    let (worktree_path, session_label) = {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let project = storage
            .load_project(project_uuid)
            .map_err(|e| e.to_string())?;

        if project.name != "the-controller" {
            return Err("Staging is only supported for the-controller".to_string());
        }

        if project.staged_session.is_some() {
            return Err("A session is already staged — unstage it first".to_string());
        }

        let session = project
            .sessions
            .iter()
            .find(|s| s.id == session_uuid)
            .ok_or("Session not found")?;

        let worktree_path = session
            .worktree_path
            .as_deref()
            .ok_or("Session has no worktree path")?
            .to_string();

        (worktree_path, session.label.clone())
    };

    // Phase 1: Ensure worktree is clean (unchanged from stage_session_inplace)
    {
        let wt = worktree_path.clone();
        let is_clean = tokio::task::spawn_blocking(move || WorktreeManager::is_worktree_clean(&wt))
            .await
            .map_err(|e| format!("Task failed: {}", e))??;

        if !is_clean {
            let prompt = "\nYou have uncommitted changes. Please commit all your work now.\r";
            {
                let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
                let _ = pty_manager.write_to_session(session_uuid, prompt.as_bytes());
            }

            let _ = state
                .emitter
                .emit("staging-status", "Waiting for commit...");

            let max_polls = MAX_COMMIT_WAIT_SECS / COMMIT_POLL_INTERVAL_SECS;
            let mut committed = false;
            for _ in 0..max_polls {
                tokio::time::sleep(std::time::Duration::from_secs(COMMIT_POLL_INTERVAL_SECS)).await;
                let wt_check = worktree_path.clone();
                let clean = tokio::task::spawn_blocking(move || {
                    WorktreeManager::is_worktree_clean(&wt_check)
                })
                .await
                .map_err(|e| format!("Task failed: {}", e))??;
                if clean {
                    committed = true;
                    break;
                }
            }
            if !committed {
                return Err(
                    "Timed out waiting for commit. Please commit manually and retry.".to_string(),
                );
            }
        }
    }

    // Phase 2: Rebase onto main if needed (unchanged)
    {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let project = storage
            .load_project(project_uuid)
            .map_err(|e| e.to_string())?;
        let repo_path = project.repo_path.clone();
        let session = project
            .sessions
            .iter()
            .find(|s| s.id == session_uuid)
            .ok_or("Session not found")?;
        let branch = session
            .worktree_branch
            .as_deref()
            .ok_or("Session has no worktree branch")?
            .to_string();
        drop(storage);

        let rp = repo_path.clone();
        let main_branch =
            tokio::task::spawn_blocking(move || WorktreeManager::detect_main_branch(&rp))
                .await
                .map_err(|e| format!("Task failed: {}", e))??;

        let rp = repo_path.clone();
        let _ = tokio::task::spawn_blocking(move || WorktreeManager::sync_main(&rp)).await;

        let rp = repo_path.clone();
        let br = branch.clone();
        let mb = main_branch.clone();
        let is_behind =
            tokio::task::spawn_blocking(move || WorktreeManager::is_branch_behind(&rp, &br, &mb))
                .await
                .map_err(|e| format!("Task failed: {}", e))??;

        if is_behind {
            let wt = worktree_path.clone();
            let mb = main_branch.clone();
            let rebase_clean =
                tokio::task::spawn_blocking(move || WorktreeManager::rebase_onto(&wt, &mb))
                    .await
                    .map_err(|e| format!("Task failed: {}", e))??;

            if !rebase_clean {
                let prompt = "\nThere are rebase conflicts. Please resolve all conflicts, then run `git rebase --continue`.\r";
                {
                    let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
                    let _ = pty_manager.write_to_session(session_uuid, prompt.as_bytes());
                }

                let _ = state
                    .emitter
                    .emit("staging-status", "Rebase conflicts. Claude is resolving...");

                let max_polls = MAX_REBASE_WAIT_SECS / REBASE_POLL_INTERVAL_SECS;
                let mut resolved = false;
                for _ in 0..max_polls {
                    tokio::time::sleep(std::time::Duration::from_secs(REBASE_POLL_INTERVAL_SECS))
                        .await;
                    let wt_check = worktree_path.clone();
                    let still_rebasing = tokio::task::spawn_blocking(move || {
                        WorktreeManager::is_rebase_in_progress(&wt_check)
                    })
                    .await
                    .map_err(|e| format!("Task failed: {}", e))?;
                    if !still_rebasing {
                        resolved = true;
                        break;
                    }
                }
                if !resolved {
                    return Err("Timed out waiting for rebase conflict resolution.".to_string());
                }
            }
        }
    }

    // Phase 3: npm install + launch dev server
    let _ = state
        .emitter
        .emit("staging-status", "Preparing staged instance...");

    // Check for node_modules, install if missing
    let node_modules = std::path::Path::new(&worktree_path).join("node_modules");
    if !node_modules.exists() {
        let _ = state
            .emitter
            .emit("staging-status", "Installing dependencies...");
        let wt = worktree_path.clone();
        let install_result = tokio::task::spawn_blocking(move || {
            std::process::Command::new("npm")
                .arg("install")
                .current_dir(&wt)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .status()
        })
        .await
        .map_err(|e| format!("Task failed: {}", e))?
        .map_err(|e| format!("npm install failed: {}", e))?;

        if !install_result.success() {
            return Err("npm install failed in worktree".to_string());
        }
    }

    // Find a free port
    let port = find_staging_port(1420)?;

    let _ = state
        .emitter
        .emit("staging-status", &format!("Starting on port {}...", port));

    // Spawn dev.sh as a new process group
    let child = std::process::Command::new("bash")
        .arg("./dev.sh")
        .arg(port.to_string())
        .current_dir(&worktree_path)
        .env("CONTROLLER_SOCKET", "/tmp/the-controller-staged.sock")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0) // Create new process group
        .spawn()
        .map_err(|e| format!("Failed to spawn staged instance: {}", e))?;

    let pid = child.id();

    // Save staged session state
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage
        .load_project(project_uuid)
        .map_err(|e| e.to_string())?;

    project.staged_session = Some(StagedSession {
        session_id: session_uuid,
        pid,
        port,
    });

    storage.save_project(&project).map_err(|e| e.to_string())?;

    Ok(())
}
```

**Step 2: Implement `unstage_session` command**

```rust
#[tauri::command]
pub fn unstage_session(state: State<AppState>, project_id: String) -> Result<(), String> {
    let project_uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage
        .load_project(project_uuid)
        .map_err(|e| e.to_string())?;

    let staged = project
        .staged_session
        .take()
        .ok_or("No session is currently staged")?;

    // Kill the process group
    kill_process_group(staged.pid);

    // Clean up the staged socket
    let _ = std::fs::remove_file("/tmp/the-controller-staged.sock");

    storage.save_project(&project).map_err(|e| e.to_string())?;
    Ok(())
}

/// Kill a process group by PID. Sends SIGTERM, then SIGKILL after 2s.
fn kill_process_group(pid: u32) {
    use std::process::Command;
    // Kill the process group (negative PID)
    let pgid = format!("-{}", pid);
    let _ = Command::new("kill")
        .args(["--", &pgid])
        .status();
    // Give processes 2s to exit, then force kill
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let _ = Command::new("kill")
            .args(["-9", "--", &pgid])
            .status();
    });
}
```

**Step 3: Register new commands in `lib.rs`**

In `src-tauri/src/lib.rs`, replace the two command registrations:

Replace:
```rust
commands::stage_session_inplace,
commands::unstage_session_inplace,
```

With:
```rust
commands::stage_session,
commands::unstage_session,
```

**Step 4: Add app exit cleanup for staged processes**

In `src-tauri/src/lib.rs`, in the `RunEvent::ExitRequested` handler (~line 132-148), add cleanup for staged processes:

```rust
if let tauri::RunEvent::ExitRequested { .. } = event {
    status_socket::cleanup();
    // Kill any staged controller instance
    if let Some(state) = app_handle.try_state::<state::AppState>() {
        if let Ok(storage) = state.storage.lock() {
            if let Ok(inventory) = storage.list_projects() {
                for project in &inventory.projects {
                    if let Some(staged) = &project.staged_session {
                        commands::kill_process_group(staged.pid);
                        let _ = std::fs::remove_file("/tmp/the-controller-staged.sock");
                    }
                }
            }
        }
    }
    // existing tmux cleanup...
```

Make `kill_process_group` pub so `lib.rs` can call it.

**Step 5: Run Rust compilation check**

Run: `cd src-tauri && cargo check`
Expected: PASS (may have warnings about unused old code in worktree.rs)

**Step 6: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: replace in-place staging with separate controller instance"
```

---

### Task 4: Make status socket path configurable

**Files:**
- Modify: `src-tauri/src/status_socket.rs:11`

**Step 1: Update socket path resolution**

Replace the constant and `socket_path()` function:

```rust
/// Return the socket path, checking the CONTROLLER_SOCKET env var first.
pub fn socket_path() -> String {
    std::env::var("CONTROLLER_SOCKET")
        .unwrap_or_else(|_| "/tmp/the-controller.sock".to_string())
}
```

**Step 2: Update all callers of `socket_path()`**

Search for uses of `socket_path()` and `SOCKET_PATH` — update them to use the function's return value (which is now a `String` instead of `&'static str`). The main places are:
- `start_listener()` in `status_socket.rs` — uses `socket_path()` for bind
- `cleanup()` in `status_socket.rs` — uses `SOCKET_PATH` for remove
- `session_args.rs` — passes socket path to Claude Code hooks

**Step 3: Run compilation check**

Run: `cd src-tauri && cargo check`
Expected: PASS

**Step 4: Commit**

```bash
git add src-tauri/src/status_socket.rs src-tauri/src/session_args.rs
git commit -m "feat: make status socket path configurable via CONTROLLER_SOCKET env var"
```

---

### Task 5: Remove old in-place staging code from worktree.rs

**Files:**
- Modify: `src-tauri/src/worktree.rs` (remove `stage_inplace`, `unstage_inplace`, `touch_changed_files`)

**Step 1: Delete the functions**

Remove:
- `stage_inplace` (lines ~335-401)
- `unstage_inplace` (lines ~405-446)
- `touch_changed_files` helper (find and remove)

**Step 2: Run compilation check**

Run: `cd src-tauri && cargo check`
Expected: PASS — nothing should reference these functions anymore

**Step 3: Run all Rust tests**

Run: `cd src-tauri && cargo test`
Expected: ALL PASS

**Step 4: Commit**

```bash
git add src-tauri/src/worktree.rs
git commit -m "refactor: remove old in-place staging functions from worktree.rs"
```

---

### Task 6: Update frontend types and stores

**Files:**
- Modify: `src/lib/stores.ts:97-101` (StagedSession interface)
- Modify: `src/lib/stores.ts:272-273` (HotkeyAction types)

**Step 1: Update `StagedSession` interface**

Replace in `src/lib/stores.ts`:

```typescript
export interface StagedSession {
  session_id: string;
  pid: number;
  port: number;
}
```

**Step 2: Update HotkeyAction types**

Replace:
```typescript
  | { type: "stage-session-inplace"; sessionId: string; projectId: string }
  | { type: "unstage-session-inplace"; projectId: string }
```

With:
```typescript
  | { type: "stage-session"; sessionId: string; projectId: string }
  | { type: "unstage-session"; projectId: string }
```

**Step 3: Run type check**

Run: `npx tsc --noEmit`
Expected: FAIL — will surface all frontend files that need updating (good, we'll fix them next)

**Step 4: Commit**

```bash
git add src/lib/stores.ts
git commit -m "refactor: update StagedSession frontend types to match new model"
```

---

### Task 7: Update frontend hotkey handler and sidebar

**Files:**
- Modify: `src/lib/HotkeyManager.svelte:330-341`
- Modify: `src/lib/Sidebar.svelte:171-176, 418-450`
- Modify: `src/lib/commands.ts:35, 79`

**Step 1: Update commands.ts**

In `src/lib/commands.ts`, rename the command ID and update description:

Replace `"stage-inplace"` with `"stage"` in the `CommandId` type (line 35).

Update the command definition (line 79):
```typescript
{ id: "stage", key: "v", section: "Sessions", description: "Stage/unstage session as separate instance", mode: "development" },
```

**Step 2: Update HotkeyManager.svelte**

Replace the `case "stage-inplace":` block (lines 330-341):

```typescript
case "stage": {
    const stageProj = projectList.find((p) => p.staged_session !== null);
    if (stageProj) {
        dispatchHotkeyAction({ type: "unstage-session", projectId: stageProj.id });
    } else if (activeId) {
        const proj2 = projectList.find((p) => p.sessions.some((s) => s.id === activeId));
        if (proj2 && proj2.name === "the-controller") {
            dispatchHotkeyAction({ type: "stage-session", sessionId: activeId, projectId: proj2.id });
        }
    }
    return true;
}
```

**Step 3: Update Sidebar.svelte action handler**

Replace the action cases (lines 171-177):

```typescript
case "stage-session": {
    stageSession(action.projectId, action.sessionId);
    break;
}
case "unstage-session": {
    unstageSession(action.projectId);
    break;
}
```

**Step 4: Replace staging functions in Sidebar.svelte**

Replace `stageSessionInplace` (lines 418-440) with:

```typescript
async function stageSession(projectId: string, sessionId: string) {
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
        unlistenStatus?.();
    }
}
```

Replace `unstageSessionInplace` (lines 442-450) with:

```typescript
async function unstageSession(projectId: string) {
    try {
        await command("unstage_session", { projectId });
        await loadProjects();
        showToast("Unstaged — stopped separate instance", "info");
    } catch (e) {
        showToast(String(e), "error");
    }
}
```

**Step 5: Run type check**

Run: `npx tsc --noEmit`
Expected: PASS (or only unrelated warnings)

**Step 6: Commit**

```bash
git add src/lib/commands.ts src/lib/HotkeyManager.svelte src/lib/Sidebar.svelte
git commit -m "feat: update frontend to use new stage/unstage commands"
```

---

### Task 8: Update title bar to show staged session

**Files:**
- Modify: `src/App.svelte:408-423`

**Step 1: Update the reactive title bar effect**

Replace the `$effect` block for staging title (lines 408-423):

```typescript
$effect(() => {
    const stagedProject = projectsState.current.find((p) => p.staged_session);
    if (stagedProject) {
        const session = stagedProject.sessions.find(
            (s) => s.id === stagedProject.staged_session!.session_id,
        );
        const label = session?.label ?? "unknown";
        const port = stagedProject.staged_session!.port;
        updateWindowTitle(
            __BUILD_BRANCH__,
            __BUILD_COMMIT__,
            `staging: ${label} (localhost:${port})`,
        );
    } else {
        updateWindowTitle(__BUILD_BRANCH__, __BUILD_COMMIT__);
    }
});
```

**Step 2: Update `updateWindowTitle` to accept optional staging info**

Replace the function (lines 331-339):

```typescript
function updateWindowTitle(branch: string, commit: string, staging?: string) {
    try {
        const parts = [commit, branch, `localhost:${__DEV_PORT__}`];
        let title = `The Controller (${parts.join(", ")})`;
        if (staging) {
            title += ` — ${staging}`;
        }
        getCurrentWindow().setTitle(title);
    } catch {
        // Browser mode — no Tauri window API available
    }
}
```

**Step 3: Run type check**

Run: `npx tsc --noEmit`
Expected: PASS

**Step 4: Commit**

```bash
git add src/App.svelte
git commit -m "feat: show staged session in controller title bar"
```

---

### Task 9: Fix tests

**Files:**
- Modify: `src/lib/Sidebar.test.ts`
- Modify: `src/lib/HotkeyManager.test.ts`
- Modify: Any other test files referencing `staged_session` with old fields

**Step 1: Update mock data in all test files**

The `staged_session: null` references in test files are fine. But any test that constructs a non-null `StagedSession` needs updating. Search for `original_branch` and `staging_branch` in test files and update to `pid` and `port`.

**Step 2: Run frontend tests**

Run: `npx vitest run`
Expected: ALL PASS

**Step 3: Run Rust tests**

Run: `cd src-tauri && cargo test`
Expected: ALL PASS

**Step 4: Commit**

```bash
git add -A
git commit -m "test: fix tests for new staging model"
```

---

### Task 10: End-to-end validation

**Step 1: Start the dev server**

Run: `npm run tauri dev`

**Step 2: Test staging**

1. Focus a session in `the-controller` project
2. Press `v` — observe:
   - Toast messages for progress (commit check, npm install if needed, port assignment)
   - Title bar updates to show "staging: session-label (localhost:2420)"
   - Sidebar shows "staged" badge
   - A new Controller window opens after cargo compilation
3. Press `v` again — observe:
   - Staged instance is killed
   - Title bar restores to normal
   - Toast: "Unstaged — stopped separate instance"

**Step 3: Test app exit cleanup**

1. Stage a session
2. Quit the main Controller
3. Verify no orphan dev.sh / cargo processes remain: `ps aux | grep dev.sh`

**Step 4: Commit any fixes needed**

```bash
git add -A
git commit -m "fix: address issues found during staging e2e validation"
```
