# Auto-rebase Before Staging — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Automatically rebase worktree branches onto main before staging, using Claude to resolve conflicts if needed.

**Architecture:** Add two helpers to `WorktreeManager` (check if branch is behind main, check if worktree is clean) and a rebase step. Modify `stage_session_inplace` in `commands.rs` to call these before staging, sending prompts to the session's Claude agent when uncommitted changes or conflicts block the rebase.

**Tech Stack:** Rust (git2, std::process::Command), Tauri PTY manager

---

### Task 1: Add `is_worktree_clean` helper to WorktreeManager

**Files:**
- Modify: `src-tauri/src/worktree.rs`

**Step 1: Write the failing test**

Add to the `tests` module in `worktree.rs`:

```rust
#[test]
fn test_is_worktree_clean_on_clean_worktree() {
    let (_tmp, repo_path) = setup_test_repo();
    let wt_dir = TempDir::new().expect("create wt temp dir");
    let worktree_dir = wt_dir.path().join("clean-wt");

    let wt_path = WorktreeManager::create_worktree(&repo_path, "clean-wt", &worktree_dir)
        .expect("create worktree");

    assert!(WorktreeManager::is_worktree_clean(wt_path.to_str().unwrap()).unwrap());
}

#[test]
fn test_is_worktree_clean_with_uncommitted_changes() {
    let (_tmp, repo_path) = setup_test_repo();
    let wt_dir = TempDir::new().expect("create wt temp dir");
    let worktree_dir = wt_dir.path().join("dirty-wt");

    let wt_path = WorktreeManager::create_worktree(&repo_path, "dirty-wt", &worktree_dir)
        .expect("create worktree");

    std::fs::write(wt_path.join("dirty.txt"), "uncommitted").unwrap();
    assert!(!WorktreeManager::is_worktree_clean(wt_path.to_str().unwrap()).unwrap());
}
```

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test is_worktree_clean -- --nocapture`
Expected: FAIL — `is_worktree_clean` method doesn't exist.

**Step 3: Write minimal implementation**

Add to `impl WorktreeManager` in `worktree.rs`, before `stage_inplace`:

```rust
/// Check if a worktree has a clean working tree (no uncommitted or untracked changes).
pub fn is_worktree_clean(worktree_path: &str) -> Result<bool, String> {
    let repo = Repository::open(worktree_path)
        .map_err(|e| format!("failed to open worktree repo: {}", e))?;
    let statuses = repo
        .statuses(Some(
            git2::StatusOptions::new()
                .include_untracked(true)
                .recurse_untracked_dirs(false),
        ))
        .map_err(|e| format!("failed to check worktree status: {}", e))?;
    Ok(statuses.is_empty())
}
```

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test is_worktree_clean -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/worktree.rs
git commit -m "feat: add is_worktree_clean helper to WorktreeManager"
```

---

### Task 2: Add `is_branch_behind` helper to WorktreeManager

**Files:**
- Modify: `src-tauri/src/worktree.rs`

**Step 1: Write the failing test**

Add to the `tests` module in `worktree.rs`:

```rust
#[test]
fn test_is_branch_behind_when_at_same_commit() {
    let (_tmp, repo_path) = setup_test_repo();
    let wt_dir = TempDir::new().expect("create wt temp dir");
    let worktree_dir = wt_dir.path().join("behind-test");

    WorktreeManager::create_worktree(&repo_path, "behind-test", &worktree_dir)
        .expect("create worktree");

    let main = WorktreeManager::detect_main_branch(&repo_path).unwrap();
    assert!(!WorktreeManager::is_branch_behind(&repo_path, "behind-test", &main).unwrap());
}

#[test]
fn test_is_branch_behind_when_main_has_new_commits() {
    let (_tmp, repo_path) = setup_test_repo();
    let wt_dir = TempDir::new().expect("create wt temp dir");
    let worktree_dir = wt_dir.path().join("behind-test2");

    WorktreeManager::create_worktree(&repo_path, "behind-test2", &worktree_dir)
        .expect("create worktree");

    // Add a commit to main so the worktree branch is behind
    let repo = Repository::open(&repo_path).unwrap();
    let sig = repo.signature().unwrap_or_else(|_| {
        git2::Signature::now("Test", "test@example.com").unwrap()
    });
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    let tree = head.tree().unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "new commit on main", &tree, &[&head])
        .unwrap();

    let main = WorktreeManager::detect_main_branch(&repo_path).unwrap();
    assert!(WorktreeManager::is_branch_behind(&repo_path, "behind-test2", &main).unwrap());
}
```

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test is_branch_behind -- --nocapture`
Expected: FAIL — `is_branch_behind` method doesn't exist.

**Step 3: Write minimal implementation**

Add to `impl WorktreeManager` in `worktree.rs`, after `is_worktree_clean`:

```rust
/// Check if `branch` is behind `main_branch` (i.e. main has commits not in branch).
pub fn is_branch_behind(repo_path: &str, branch: &str, main_branch: &str) -> Result<bool, String> {
    let repo = Repository::open(repo_path)
        .map_err(|e| format!("failed to open repo: {}", e))?;

    let branch_commit = repo
        .find_branch(branch, git2::BranchType::Local)
        .map_err(|e| format!("branch '{}' not found: {}", branch, e))?
        .get()
        .peel_to_commit()
        .map_err(|e| format!("failed to resolve branch commit: {}", e))?
        .id();

    let main_commit = repo
        .find_branch(main_branch, git2::BranchType::Local)
        .map_err(|e| format!("branch '{}' not found: {}", main_branch, e))?
        .get()
        .peel_to_commit()
        .map_err(|e| format!("failed to resolve main commit: {}", e))?
        .id();

    if branch_commit == main_commit {
        return Ok(false);
    }

    let merge_base = repo
        .merge_base(branch_commit, main_commit)
        .map_err(|e| format!("failed to find merge base: {}", e))?;

    // Branch is behind if its tip equals the merge base but main is ahead
    Ok(merge_base == branch_commit && main_commit != branch_commit)
}
```

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test is_branch_behind -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/worktree.rs
git commit -m "feat: add is_branch_behind helper to WorktreeManager"
```

---

### Task 3: Add `rebase_onto` helper to WorktreeManager

**Files:**
- Modify: `src-tauri/src/worktree.rs`

**Step 1: Write the failing test**

Add to the `tests` module in `worktree.rs`:

```rust
#[test]
fn test_rebase_onto_succeeds_when_no_conflicts() {
    let (_tmp, repo_path) = setup_test_repo();
    let wt_dir = TempDir::new().expect("create wt temp dir");
    let worktree_dir = wt_dir.path().join("rebase-test");

    let wt_path = WorktreeManager::create_worktree(&repo_path, "rebase-test", &worktree_dir)
        .expect("create worktree");

    // Add a commit to main
    let repo = Repository::open(&repo_path).unwrap();
    let sig = repo.signature().unwrap_or_else(|_| {
        git2::Signature::now("Test", "test@example.com").unwrap()
    });
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    let mut index = repo.index().unwrap();
    std::fs::write(Path::new(&repo_path).join("main-file.txt"), "from main").unwrap();
    index.add_path(Path::new("main-file.txt")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "main commit", &tree, &[&head]).unwrap();

    // Add a non-conflicting commit to worktree
    let wt_repo = Repository::open(&wt_path).unwrap();
    let wt_sig = wt_repo.signature().unwrap_or_else(|_| {
        git2::Signature::now("Test", "test@example.com").unwrap()
    });
    let wt_head = wt_repo.head().unwrap().peel_to_commit().unwrap();
    std::fs::write(wt_path.join("wt-file.txt"), "from worktree").unwrap();
    let mut wt_index = wt_repo.index().unwrap();
    wt_index.add_path(Path::new("wt-file.txt")).unwrap();
    wt_index.write().unwrap();
    let wt_tree_id = wt_index.write_tree().unwrap();
    let wt_tree = wt_repo.find_tree(wt_tree_id).unwrap();
    wt_repo.commit(Some("HEAD"), &wt_sig, &wt_sig, "wt commit", &wt_tree, &[&wt_head]).unwrap();

    let main = WorktreeManager::detect_main_branch(&repo_path).unwrap();
    let result = WorktreeManager::rebase_onto(wt_path.to_str().unwrap(), &main);
    assert!(result.is_ok(), "rebase should succeed: {:?}", result);
    assert!(result.unwrap(), "rebase should return true (success)");

    // Verify worktree has both files after rebase
    assert!(wt_path.join("main-file.txt").exists());
    assert!(wt_path.join("wt-file.txt").exists());
}
```

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test rebase_onto -- --nocapture`
Expected: FAIL — `rebase_onto` method doesn't exist.

**Step 3: Write minimal implementation**

Add to `impl WorktreeManager` in `worktree.rs`, after `is_branch_behind`:

```rust
/// Rebase the worktree's current branch onto `main_branch`.
/// Returns `Ok(true)` if rebase succeeded, `Ok(false)` if there were conflicts
/// (rebase left in progress for Claude to resolve).
pub fn rebase_onto(worktree_path: &str, main_branch: &str) -> Result<bool, String> {
    let output = Command::new("git")
        .args(["rebase", main_branch])
        .current_dir(worktree_path)
        .output()
        .map_err(|e| format!("failed to run git rebase: {}", e))?;

    if output.status.success() {
        Ok(true)
    } else {
        // Check if rebase is in progress (conflicts) vs outright failure
        if Self::is_rebase_in_progress(worktree_path) {
            Ok(false)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("git rebase failed: {}", stderr.trim()))
        }
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test rebase_onto -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/worktree.rs
git commit -m "feat: add rebase_onto helper to WorktreeManager"
```

---

### Task 4: Add auto-rebase logic to `stage_session_inplace` command

This is the integration task. Modify the Tauri command to check cleanliness, check if behind, rebase if needed, and send prompts to Claude on failure.

**Files:**
- Modify: `src-tauri/src/commands.rs` (the `stage_session_inplace` function, lines ~586-628 on master)

**Step 1: Write the updated command**

Replace the existing `stage_session_inplace` function. Key changes:
- Extract `worktree_path` from the session
- Check if worktree is clean; if not, send Claude a "commit" prompt and return error
- Sync main before checking if behind
- Check if branch is behind main; if so, rebase
- If rebase has conflicts, send Claude a "resolve conflicts" prompt and return error
- If rebase succeeds or branch was up to date, proceed with normal staging

```rust
#[tauri::command]
pub fn stage_session_inplace(
    state: State<AppState>,
    project_id: String,
    session_id: String,
) -> Result<(), String> {
    use crate::models::StagedSession;

    let project_uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let session_uuid = Uuid::parse_str(&session_id).map_err(|e| e.to_string())?;

    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage.load_project(project_uuid).map_err(|e| e.to_string())?;

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
        .ok_or("Session has no worktree branch")?;

    let worktree_path = session
        .worktree_path
        .as_deref()
        .ok_or("Session has no worktree path")?;

    // 1. Check worktree is clean
    if !WorktreeManager::is_worktree_clean(worktree_path)? {
        // Send prompt to Claude to commit changes
        let prompt = "You have uncommitted changes. Please commit all your work now.\r";
        {
            let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
            let _ = pty_manager.write_to_session(session_uuid, prompt.as_bytes());
        }
        return Err("Worktree has uncommitted changes — asked Claude to commit. Retry staging after.".to_string());
    }

    // 2. Check if branch is behind main and rebase if needed
    let main_branch = WorktreeManager::detect_main_branch(&project.repo_path)?;

    // Sync main first so we rebase onto latest
    let _ = WorktreeManager::sync_main(&project.repo_path);

    if WorktreeManager::is_branch_behind(&project.repo_path, branch, &main_branch)? {
        match WorktreeManager::rebase_onto(worktree_path, &main_branch) {
            Ok(true) => {
                // Rebase succeeded, continue to staging
            }
            Ok(false) => {
                // Conflicts — ask Claude to resolve, block staging
                let prompt = "There are rebase conflicts. Please resolve all conflicts, then run `git rebase --continue`.\r";
                {
                    let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
                    let _ = pty_manager.write_to_session(session_uuid, prompt.as_bytes());
                }
                return Err("Rebase conflicts — asked Claude to resolve. Retry staging after.".to_string());
            }
            Err(e) => return Err(format!("Rebase failed: {}", e)),
        }
    }

    // 3. Proceed with staging
    let original_branch =
        WorktreeManager::stage_inplace(&project.repo_path, branch)?;

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

**Step 2: Run all tests to verify nothing is broken**

Run: `cd src-tauri && cargo test`
Expected: PASS (all existing tests still pass)

**Step 3: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "feat: auto-rebase worktree branch before staging"
```

---

### Task 5: Manual smoke test

Since this involves PTY interaction and the full Tauri app, do a manual smoke test:

1. Run `npm run tauri dev`
2. Create a session, make a commit in the worktree
3. Add a commit to main so the worktree is behind
4. Press `v` — should see rebase happen, then staging succeeds
5. Press `v` again — should unstage normally
6. Test with uncommitted changes — should see toast error and Claude prompted to commit
