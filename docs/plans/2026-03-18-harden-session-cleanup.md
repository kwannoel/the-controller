# Harden Session Cleanup Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan task-by-task.

**Goal:** Eliminate orphaned worktree directories and git branches by making `remove_worktree` resilient (continue all steps even if one fails) and adding backend-driven cleanup after merge.

**Architecture:** Two changes: (1) Make `remove_worktree` best-effort on each step independently so a prune failure doesn't skip branch deletion, and propagate errors to callers instead of `let _`. (2) After `merge_session_branch` succeeds, call `close_session` to clean up the worktree/branch/session — don't rely on Claude sending a socket signal. Same for the `finish-branch` hotkey path: listen for the socket cleanup event and fall back to calling `close_session` if it doesn't arrive within a timeout.

**Tech Stack:** Rust (Tauri v2), Svelte 5 (TypeScript), git2

---

### Task 1: Make `remove_worktree` resilient — continue all steps independently

**Why:** Currently, `remove_worktree` uses `?` after `remove_dir_all` and after `wt.prune()`. If either fails, it early-returns and skips subsequent steps. The branch is never deleted if prune fails. Each step should execute independently and errors should be collected.

**Files:**
- Modify: `src-tauri/src/worktree.rs:375-420`
- Test: `src-tauri/src/worktree.rs` (existing test module)

**Step 1: Write the failing test**

Add to the existing `mod tests` in `worktree.rs`:

```rust
#[test]
fn test_remove_worktree_deletes_branch_even_when_dir_already_gone() {
    let (_tmp, repo_path) = setup_test_repo();
    let wt_dir = TempDir::new().expect("create wt temp dir");
    let worktree_dir = wt_dir.path().join("partial-cleanup");

    // Create a worktree (creates dir + git ref + branch)
    let wt_path = WorktreeManager::create_worktree(&repo_path, "partial-cleanup", &worktree_dir)
        .expect("create worktree");

    // Simulate partial cleanup: manually delete the directory
    std::fs::remove_dir_all(&wt_path).expect("manually remove dir");

    // remove_worktree should still clean up the branch
    let result = WorktreeManager::remove_worktree(
        wt_path.to_str().unwrap(),
        &repo_path,
        "partial-cleanup",
    );
    // Should succeed (or at least not block on the missing dir)
    assert!(result.is_ok(), "remove_worktree should not fail: {:?}", result);

    // Branch should be gone
    let repo = git2::Repository::open(&repo_path).unwrap();
    let branch = repo.find_branch("partial-cleanup", git2::BranchType::Local);
    assert!(branch.is_err(), "branch should have been deleted");
}
```

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test test_remove_worktree_deletes_branch_even_when_dir_already_gone -- --nocapture`
Expected: PASS (this specific case already works because `remove_dir_all` on a nonexistent dir is guarded by `if worktree_dir.exists()`)

Actually, the real gap is when `prune` fails. Let's write a test that validates the branch is deleted even when there's no worktree git reference to prune. Add a second test:

```rust
#[test]
fn test_remove_worktree_branch_deleted_even_if_prune_would_fail() {
    let (_tmp, repo_path) = setup_test_repo();
    let wt_dir = TempDir::new().expect("create wt temp dir");
    let worktree_dir = wt_dir.path().join("prune-fail");

    // Create a worktree
    let wt_path = WorktreeManager::create_worktree(&repo_path, "prune-fail", &worktree_dir)
        .expect("create worktree");

    // Manually prune the worktree reference (simulating partial cleanup)
    let repo = git2::Repository::open(&repo_path).unwrap();
    let wt = repo.find_worktree("prune-fail").unwrap();
    let mut prune_opts = git2::WorktreePruneOptions::new();
    prune_opts.valid(true);
    prune_opts.working_tree(true);
    wt.prune(Some(&mut prune_opts)).unwrap();

    // Now remove_worktree — prune will find nothing, but branch should still be deleted
    let result = WorktreeManager::remove_worktree(
        wt_path.to_str().unwrap(),
        &repo_path,
        "prune-fail",
    );
    assert!(result.is_ok(), "should succeed: {:?}", result);

    // Branch must be gone
    let repo = git2::Repository::open(&repo_path).unwrap();
    let branch = repo.find_branch("prune-fail", git2::BranchType::Local);
    assert!(branch.is_err(), "branch should have been deleted");
}
```

**Step 3: Run tests to verify they pass (baseline)**

Run: `cd src-tauri && cargo test test_remove_worktree_branch_deleted_even_if_prune_would_fail -- --nocapture`
Expected: PASS (prune is already guarded by `if let Ok(wt)`)

Now the critical test — verify that a prune _error_ (not just missing) blocks branch deletion with the current code:

```rust
#[test]
fn test_remove_worktree_collects_all_errors() {
    let (_tmp, repo_path) = setup_test_repo();

    // Call remove_worktree with a bogus path and valid repo but no matching branch
    let result = WorktreeManager::remove_worktree(
        "/tmp/does-not-exist-at-all",
        &repo_path,
        "no-such-branch",
    );

    // Current behavior: returns Ok(()) because all guards skip gracefully.
    // After our change: should still return Ok(()) — no actual errors, just skipped steps.
    assert!(result.is_ok());
}
```

**Step 4: Implement the resilient version**

Replace `remove_worktree` in `src-tauri/src/worktree.rs:375-420` with:

```rust
pub fn remove_worktree(
    worktree_path: &str,
    repo_path: &str,
    branch_name: &str,
) -> Result<(), String> {
    tracing::debug!(
        branch = branch_name,
        path = worktree_path,
        "removing worktree"
    );

    let mut errors: Vec<String> = Vec::new();

    // Step 1: Remove the worktree directory if it exists
    let worktree_dir = Path::new(worktree_path);
    if worktree_dir.exists() {
        tracing::debug!(path = worktree_path, "removing worktree directory from disk");
        if let Err(e) = std::fs::remove_dir_all(worktree_dir) {
            let msg = format!("failed to remove worktree dir: {}", e);
            tracing::error!("{}", msg);
            errors.push(msg);
        }
    }

    // Step 2: Open repo (needed for prune + branch delete)
    let repo = match Repository::open(repo_path) {
        Ok(r) => Some(r),
        Err(e) => {
            let msg = format!("failed to open repo: {}", e);
            tracing::error!("{}", msg);
            errors.push(msg);
            None
        }
    };

    if let Some(repo) = &repo {
        // Step 3: Prune the worktree reference
        if let Ok(wt) = repo.find_worktree(branch_name) {
            tracing::debug!(branch = branch_name, "pruning worktree git reference");
            let mut prune_opts = git2::WorktreePruneOptions::new();
            prune_opts.valid(true);
            prune_opts.working_tree(true);
            if let Err(e) = wt.prune(Some(&mut prune_opts)) {
                let msg = format!("failed to prune worktree: {}", e);
                tracing::error!("{}", msg);
                errors.push(msg);
            }
        }

        // Step 4: Delete the branch (always attempted, regardless of prune result)
        if let Ok(mut branch) = repo.find_branch(branch_name, git2::BranchType::Local) {
            tracing::debug!(branch = branch_name, "deleting branch after worktree removal");
            if let Err(e) = branch.delete() {
                let msg = format!("failed to delete branch: {}", e);
                tracing::error!("{}", msg);
                errors.push(msg);
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!("worktree cleanup had errors: {}", errors.join("; ")))
    }
}
```

**Step 5: Run all worktree tests**

Run: `cd src-tauri && cargo test worktree::tests -- --nocapture`
Expected: All existing + new tests PASS

**Step 6: Commit**

```bash
git add src-tauri/src/worktree.rs
git commit -m "fix: make remove_worktree resilient — continue all steps independently"
```

---

### Task 2: Log `remove_worktree` errors at all call sites instead of `let _`

**Why:** Every call site except `status_socket.rs` silently discards `remove_worktree` errors. This means failures are invisible. After Task 1 made `remove_worktree` return errors only when steps genuinely fail (not skip), these errors are meaningful and should be logged.

**Files:**
- Modify: `src-tauri/src/commands.rs:1558-1559` (`close_session`)
- Modify: `src-tauri/src/commands.rs:746` (`delete_project` path — find the exact line)
- Modify: `src-tauri/src/auto_worker.rs:937-941`
- Modify: `src-tauri/src/server/main.rs:708-713`

**Step 1: Replace `let _` with `if let Err` logging at each site**

In `commands.rs` `close_session` (line ~1559):
```rust
// Before:
let _ = WorktreeManager::remove_worktree(&wt_path, &project.repo_path, &branch);

// After:
if let Err(e) = WorktreeManager::remove_worktree(&wt_path, &project.repo_path, &branch) {
    tracing::error!(session_id = %session_uuid, "worktree cleanup errors: {}", e);
}
```

Apply the same pattern at every other `let _ = WorktreeManager::remove_worktree(...)` call site:
- `commands.rs` delete_project path
- `auto_worker.rs:937`
- `server/main.rs:708`

**Step 2: Run format + clippy**

Run: `cd src-tauri && cargo fmt --check && cargo clippy -- -D warnings`
Expected: Clean

**Step 3: Run all tests**

Run: `cd src-tauri && cargo test`
Expected: All PASS

**Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/auto_worker.rs src-tauri/src/server/main.rs
git commit -m "fix: log remove_worktree errors instead of silently discarding"
```

---

### Task 3: Add post-merge cleanup to `merge_session_branch`

**Why:** Currently `merge_session_branch` returns the PR URL and does nothing else. The session, worktree directory, and branch all persist. The frontend `mergeSession()` function also does no cleanup. This path is currently unreachable from any hotkey, but it's the Tauri command and should work correctly if/when it's used.

**Files:**
- Modify: `src-tauri/src/commands.rs:2053-2056` (the `PrCreated` match arm)
- Modify: `src/lib/Sidebar.svelte:533-556` (`mergeSession` function)

**Step 1: Add cleanup to the `PrCreated` arm in `merge_session_branch`**

After the `PrCreated(url)` match in `commands.rs:2053-2056`, add cleanup before returning:

```rust
crate::worktree::MergeResult::PrCreated(url) => {
    tracing::info!(session_id = %session_uuid, url = %url, "PR created, cleaning up session");

    // Clean up: kill PTY, remove session from project, delete worktree
    {
        let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
        let _ = pty_manager.close_session(session_uuid);
    }
    {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let mut project = storage.load_project(project_uuid).map_err(|e| e.to_string())?;
        project.sessions.retain(|s| s.id != session_uuid);
        storage.save_project(&project).map_err(|e| e.to_string())?;
    }
    // Delete worktree + branch
    if let Err(e) = WorktreeManager::remove_worktree(&worktree_path, &repo_path, &branch_name) {
        tracing::error!(session_id = %session_uuid, "post-merge worktree cleanup errors: {}", e);
    }

    return Ok(crate::models::MergeResponse::PrCreated { url });
}
```

Note: `worktree_path`, `repo_path`, and `branch_name` are already extracted at the top of the function (lines 2016-2035).

**Step 2: Add cleanup to the frontend `mergeSession`**

In `Sidebar.svelte:533-556`, after the successful merge, refresh the project list and emit cleanup:

```typescript
async function mergeSession(projectId: string, sessionId: string) {
    mergeInProgress = true;
    activeSessionId.set(sessionId);
    focusTerminalSoon();

    const unlistenStatus = listen<string>("merge-status", (payload) => {
      showToast(payload, "info");
    });

    try {
      const result: { type: string; url?: string } = await command("merge_session_branch", { projectId, sessionId });
      if (result.type === "pr_created") {
        showToast(`PR created: ${result.url}`, "info");
      }
      // Backend already cleaned up session — refresh frontend state
      clearSessionTracking(sessionId);
      activeSessionId.update(current => current === sessionId ? null : current);
      await loadProjects();
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      mergeInProgress = false;
      unlistenStatus?.();
    }
  }
```

**Step 3: Run format + lint**

Run: `cd src-tauri && cargo fmt --check && cargo clippy -- -D warnings`
Run: `pnpm check`
Expected: Clean

**Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src/lib/Sidebar.svelte
git commit -m "fix: clean up session after merge_session_branch succeeds"
```

---

### Task 4: Add backend-driven cleanup for the `finish-branch` (socket) path as a fallback

**Why:** The `m` hotkey sends a prompt to Claude, which is supposed to send `cleanup:<session_id>` over the socket when done. If Claude stops early, the env var is missing, or the socket is down, cleanup never happens. We need a fallback: if the session's branch has been merged into main but the session still exists, clean it up.

This is a lightweight reconciliation that runs when a session's PTY exits (the `SessionStopped` status event). If the branch is gone or merged, trigger cleanup.

**Files:**
- Modify: `src-tauri/src/worktree.rs` (add `is_branch_merged` helper)
- Modify: `src-tauri/src/status_socket.rs` (check branch state on `stopped` status)
- Test: `src-tauri/src/worktree.rs`

**Step 1: Write the failing test for `is_branch_merged`**

```rust
#[test]
fn test_is_branch_merged_into_main() {
    let (_tmp, repo_path) = setup_test_repo();
    let wt_dir = TempDir::new().expect("create wt temp dir");
    let worktree_dir = wt_dir.path().join("merge-check");

    // Create a worktree and add a commit
    let wt_path = WorktreeManager::create_worktree(&repo_path, "merge-check", &worktree_dir)
        .expect("create worktree");
    let wt_repo = Repository::open(&wt_path).unwrap();
    let sig = git2::Signature::now("Test", "test@example.com").unwrap();
    let head = wt_repo.head().unwrap().peel_to_commit().unwrap();
    let mut index = wt_repo.index().unwrap();
    std::fs::write(wt_path.join("file.txt"), "content").unwrap();
    index.add_path(Path::new("file.txt")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = wt_repo.find_tree(tree_id).unwrap();
    wt_repo.commit(Some("HEAD"), &sig, &sig, "test commit", &tree, &[&head]).unwrap();

    // Before merge: branch is NOT merged
    assert!(!WorktreeManager::is_branch_merged(&repo_path, "merge-check").unwrap());

    // Simulate merge: fast-forward main to include the branch's commit
    let repo = Repository::open(&repo_path).unwrap();
    let branch_commit = repo
        .find_branch("merge-check", git2::BranchType::Local)
        .unwrap()
        .get()
        .peel_to_commit()
        .unwrap();
    repo.find_reference("refs/heads/main")
        .or_else(|_| repo.find_reference("refs/heads/master"))
        .unwrap()
        .set_target(branch_commit.id(), "test merge")
        .unwrap();

    // After merge: branch IS merged
    assert!(WorktreeManager::is_branch_merged(&repo_path, "merge-check").unwrap());
}

#[test]
fn test_is_branch_merged_missing_branch() {
    let (_tmp, repo_path) = setup_test_repo();
    // Nonexistent branch should return true (branch is gone = already cleaned up)
    assert!(WorktreeManager::is_branch_merged(&repo_path, "nonexistent").unwrap());
}
```

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test test_is_branch_merged -- --nocapture`
Expected: FAIL — `is_branch_merged` doesn't exist yet

**Step 3: Implement `is_branch_merged`**

Add to `WorktreeManager` impl in `worktree.rs`:

```rust
/// Check if a branch has been merged into the main branch (or doesn't exist).
/// Returns true if the branch is gone or all its commits are reachable from main.
pub fn is_branch_merged(repo_path: &str, branch_name: &str) -> Result<bool, String> {
    let repo = Repository::open(repo_path).map_err(|e| format!("failed to open repo: {}", e))?;

    let branch = match repo.find_branch(branch_name, git2::BranchType::Local) {
        Ok(b) => b,
        Err(_) => return Ok(true), // Branch doesn't exist — treat as merged/cleaned
    };

    let branch_commit = branch
        .get()
        .peel_to_commit()
        .map_err(|e| format!("failed to peel branch to commit: {}", e))?;

    let main_branch_name = Self::detect_main_branch(repo_path)?;
    let main_branch = repo
        .find_branch(&main_branch_name, git2::BranchType::Local)
        .map_err(|e| format!("failed to find main branch: {}", e))?;
    let main_commit = main_branch
        .get()
        .peel_to_commit()
        .map_err(|e| format!("failed to peel main to commit: {}", e))?;

    let is_ancestor = repo
        .graph_descendant_of(main_commit.id(), branch_commit.id())
        .map_err(|e| format!("failed to check ancestry: {}", e))?;

    Ok(is_ancestor)
}
```

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test test_is_branch_merged -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/worktree.rs
git commit -m "feat: add is_branch_merged helper for post-merge cleanup detection"
```

---

### Task 5: Trigger cleanup on session stop if branch is merged

**Why:** When a session's PTY exits (`stopped` status), check if the branch was merged into main. If so, clean up automatically — this catches the case where Claude finishes the merge but the socket cleanup signal never arrives.

**Files:**
- Modify: `src-tauri/src/status_socket.rs` (add check in `stopped` handler)

**Step 1: Find where `stopped` status is handled**

Read `status_socket.rs` to find where `StatusMessage::Stopped` or the "stopped" string is processed.

**Step 2: Add post-stop branch check**

After a session reports `stopped` status, check if its branch is merged. If so, trigger the same `handle_cleanup_with_state` flow. The logic should be:

```rust
// In the stopped handler, after updating session status:
// Check if the session's branch was merged — if so, auto-cleanup
if let Ok(storage) = state.storage.lock() {
    if let Ok(inventory) = storage.list_projects() {
        for project in &inventory.projects {
            if let Some(session) = project.sessions.iter().find(|s| s.id == session_id) {
                if let Some(branch) = &session.worktree_branch {
                    match WorktreeManager::is_branch_merged(&project.repo_path, branch) {
                        Ok(true) => {
                            tracing::info!(
                                session_id = %session_id,
                                branch = %branch,
                                "branch merged into main, auto-cleaning up session"
                            );
                            // Drop storage lock before calling cleanup (which re-acquires it)
                            drop(storage);
                            handle_cleanup_with_state(state, session_id);
                            return; // cleanup done, skip normal stopped handling
                        }
                        Ok(false) => {} // Branch not merged, normal stop
                        Err(e) => {
                            tracing::warn!(
                                session_id = %session_id,
                                "failed to check if branch is merged: {}",
                                e
                            );
                        }
                    }
                }
                break;
            }
        }
    }
}
```

**Important:** The storage lock must be dropped before calling `handle_cleanup_with_state` because that function also acquires the storage lock. Restructure to avoid deadlock.

**Step 3: Run format + clippy**

Run: `cd src-tauri && cargo fmt --check && cargo clippy -- -D warnings`
Expected: Clean

**Step 4: Run all tests**

Run: `cd src-tauri && cargo test`
Expected: All PASS

**Step 5: Commit**

```bash
git add src-tauri/src/status_socket.rs
git commit -m "feat: auto-cleanup session when PTY stops and branch is merged"
```

---

### Task 6: Final validation

**Step 1: Run full test suite**

Run: `cd src-tauri && cargo test`
Run: `pnpm test`
Expected: All PASS

**Step 2: Run all lint gates**

Run: `pnpm check`
Run: `cd src-tauri && cargo fmt --check`
Run: `cd src-tauri && cargo clippy -- -D warnings`
Expected: All clean

**Step 3: Manual verification scenario**

1. Create a session with `n`, verify worktree + branch exist
2. Delete with `d` → confirm the worktree dir and branch are gone
3. Create another session, make a commit, merge with `m`
4. Verify that after Claude finishes (or after the PTY stops), the worktree dir and branch are cleaned up

**Step 4: Final commit if any fixups needed, then done**
