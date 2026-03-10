# Staging Single-Flow Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Make `v` (stage) a single action that handles commit, rebase, and staging — instead of bailing early and requiring multiple presses.

**Architecture:** Convert `stage_session_inplace` from sync to async. When worktree is dirty, prompt Claude to commit and poll until clean. When rebase hits conflicts, prompt Claude to resolve and poll until done. Mirror the existing `merge_session_branch` pattern. Frontend listens for progress events.

**Tech Stack:** Rust (Tauri async commands, tokio), Svelte 5 (Tauri event listener)

---

### Task 1: Make `stage_session_inplace` async with commit polling

**Files:**
- Modify: `src-tauri/src/commands.rs:585-672`

**Step 1: Replace the entire `stage_session_inplace` function**

Replace lines 585-672 with:

```rust
const COMMIT_POLL_INTERVAL_SECS: u64 = 3;
const MAX_COMMIT_WAIT_SECS: u64 = 60;
const MAX_STAGING_REBASE_RETRIES: u32 = 5;

#[tauri::command]
pub async fn stage_session_inplace(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    project_id: String,
    session_id: String,
) -> Result<(), String> {
    use crate::models::StagedSession;

    let project_uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let session_uuid = Uuid::parse_str(&session_id).map_err(|e| e.to_string())?;

    // Extract data under a short-lived storage lock to avoid deadlock with pty_manager
    let (repo_path, branch, worktree_path) = {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let project = storage.load_project(project_uuid).map_err(|e| e.to_string())?;

        if project.staged_session.is_some() {
            return Err("A session is already staged — unstage it first".to_string());
        }

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

        let worktree_path = session
            .worktree_path
            .as_deref()
            .ok_or("Session has no worktree path")?
            .to_string();

        (project.repo_path.clone(), branch, worktree_path)
    };

    // 1. Ensure worktree is clean — prompt Claude to commit if needed
    {
        let wt = worktree_path.clone();
        let is_clean = tokio::task::spawn_blocking(move || {
            WorktreeManager::is_worktree_clean(&wt)
        })
        .await
        .map_err(|e| format!("Task failed: {}", e))??;

        if !is_clean {
            let prompt = "\nYou have uncommitted changes. Please commit all your work now.\r";
            {
                let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
                let _ = pty_manager.write_to_session(session_uuid, prompt.as_bytes());
            }

            let _ = app_handle.emit("staging-status", "Waiting for commit...");

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
                return Err("Timed out waiting for commit. Please commit manually and retry.".to_string());
            }
        }
    }

    // 2. Rebase onto main if needed — resolve conflicts with retries
    {
        let rp = repo_path.clone();
        let main_branch = tokio::task::spawn_blocking(move || {
            WorktreeManager::detect_main_branch(&rp)
        })
        .await
        .map_err(|e| format!("Task failed: {}", e))??;

        let rp = repo_path.clone();
        let _ = tokio::task::spawn_blocking(move || {
            WorktreeManager::sync_main(&rp)
        })
        .await;

        for attempt in 0..MAX_STAGING_REBASE_RETRIES {
            let rp = repo_path.clone();
            let br = branch.clone();
            let mb = main_branch.clone();
            let is_behind = tokio::task::spawn_blocking(move || {
                WorktreeManager::is_branch_behind(&rp, &br, &mb)
            })
            .await
            .map_err(|e| format!("Task failed: {}", e))??;

            if !is_behind {
                break;
            }

            let wt = worktree_path.clone();
            let mb = main_branch.clone();
            let rebase_result = tokio::task::spawn_blocking(move || {
                WorktreeManager::rebase_onto(&wt, &mb)
            })
            .await
            .map_err(|e| format!("Task failed: {}", e))??;

            if rebase_result {
                // Rebase succeeded cleanly
                break;
            }

            // Rebase has conflicts — ask Claude to resolve
            let prompt = "\nThere are rebase conflicts. Please resolve all conflicts, then run `git rebase --continue`.\r";
            {
                let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
                let _ = pty_manager.write_to_session(session_uuid, prompt.as_bytes());
            }

            let _ = app_handle.emit(
                "staging-status",
                format!(
                    "Rebase conflicts (attempt {}/{}). Claude is resolving...",
                    attempt + 1,
                    MAX_STAGING_REBASE_RETRIES
                ),
            );

            // Poll until rebase is no longer in progress
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(REBASE_POLL_INTERVAL_SECS)).await;
                let wt_check = worktree_path.clone();
                let still_rebasing = tokio::task::spawn_blocking(move || {
                    WorktreeManager::is_rebase_in_progress(&wt_check)
                })
                .await
                .map_err(|e| format!("Task failed: {}", e))?;
                if !still_rebasing {
                    break;
                }
            }

            // Loop back — will check is_behind again
        }
    }

    // 3. Proceed with staging — re-acquire storage lock
    let rp = repo_path.clone();
    let br = branch.clone();
    let original_branch = tokio::task::spawn_blocking(move || {
        WorktreeManager::stage_inplace(&rp, &br)
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))??;

    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage.load_project(project_uuid).map_err(|e| e.to_string())?;

    let staging_branch = format!("staging/{}", branch);
    project.staged_session = Some(StagedSession {
        session_id: session_uuid,
        original_branch,
        staging_branch,
    });

    storage.save_project(&project).map_err(|e| e.to_string())?;

    Ok(())
}
```

**Step 2: Compile and verify**

Run: `cd src-tauri && cargo check`
Expected: compiles cleanly (no new dependencies needed — `AppHandle`, `Emitter`, `tokio` already available)

**Step 3: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "feat: make stage_session_inplace async with commit/rebase polling"
```

---

### Task 2: Add frontend progress events and terminal focus

**Files:**
- Modify: `src/lib/Sidebar.svelte:517-528`

**Step 1: Update `stageSessionInplace` to mirror `mergeSession` pattern**

Replace lines 517-528 with:

```typescript
  async function stageSessionInplace(projectId: string, sessionId: string) {
    // Focus the terminal so user can watch Claude commit/resolve conflicts
    activeSessionId.set(sessionId);
    focusTerminalSoon();

    // Listen for intermediate staging status events
    let unlistenStatus: (() => void) | null = null;
    listen<string>("staging-status", (event) => {
      showToast(event.payload, "info");
    }).then(fn => { unlistenStatus = fn; });

    try {
      await invoke("stage_session_inplace", { projectId, sessionId });
      await loadProjects();
      const session = projectList
        .find((p) => p.id === projectId)
        ?.sessions.find((s) => s.id === sessionId);
      showToast(`Staged ${session?.label ?? "session"} in main repo`, "info");
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      unlistenStatus?.();
    }
  }
```

**Step 2: Verify dev server compiles**

Run: `npx svelte-check`
Expected: no errors

**Step 3: Commit**

```bash
git add src/lib/Sidebar.svelte
git commit -m "feat: show staging progress toasts and focus terminal during staging"
```

---

### Task 3: Manual integration test

**Step 1: Start the app**

Run: `npm run tauri dev`

**Step 2: Test clean worktree staging (no changes)**

1. Open a session with a clean worktree
2. Press `v` to stage
3. Expected: stages immediately, shows "Staged ... in main repo" toast

**Step 3: Test dirty worktree staging (uncommitted changes)**

1. Open a session, make a change in the worktree without committing
2. Press `v` to stage
3. Expected: terminal focuses, "Waiting for commit..." toast appears, Claude commits, then staging proceeds automatically

**Step 4: Test rebase scenario**

1. Have a session whose branch is behind main
2. Press `v` to stage
3. Expected: commits if needed, rebases, shows progress toasts, stages

**Step 5: Commit if any fixes were needed**
