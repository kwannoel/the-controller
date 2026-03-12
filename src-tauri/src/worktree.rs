use git2::Repository;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Result of a merge attempt.
pub enum MergeResult {
    /// PR created successfully — contains the PR URL.
    PrCreated(String),
    /// Rebase has conflicts — worktree left in conflicted state for Claude to resolve.
    RebaseConflicts,
}

pub struct WorktreeManager;

impl WorktreeManager {
    /// Create a new git worktree for the given branch name at the specified directory.
    ///
    /// Opens the repository, creates a new branch from HEAD, and sets up
    /// the worktree at `worktree_dir`. Returns the path to the worktree directory.
    pub fn create_worktree(
        repo_path: &str,
        branch_name: &str,
        worktree_dir: &Path,
    ) -> Result<PathBuf, String> {
        let repo = Repository::open(repo_path).map_err(|e| format!("failed to open repo: {}", e))?;

        // Check if the repo has any commits (HEAD exists)
        let head = match repo.head() {
            Ok(h) => h,
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
                // Repo has no commits — can't create worktree, use repo path directly
                return Err("unborn_branch".to_string());
            }
            Err(e) => return Err(format!("failed to get HEAD: {}", e)),
        };

        if worktree_dir.exists() {
            return Err(format!(
                "worktree directory already exists: {}",
                worktree_dir.display()
            ));
        }

        // Create the parent directory
        if let Some(parent) = worktree_dir.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create worktree parent dir: {}", e))?;
        }
        let commit = head
            .peel_to_commit()
            .map_err(|e| format!("failed to peel HEAD to commit: {}", e))?;

        // Delete stale branch if it exists (left over from a previous session)
        if let Ok(mut existing) = repo.find_branch(branch_name, git2::BranchType::Local) {
            let _ = existing.delete();
        }

        let branch = repo
            .branch(branch_name, &commit, false)
            .map_err(|e| format!("failed to create branch '{}': {}", branch_name, e))?;

        // Create the worktree with the new branch as its HEAD
        let reference = branch.into_reference();
        let mut opts = git2::WorktreeAddOptions::new();
        opts.reference(Some(&reference));

        repo.worktree(branch_name, worktree_dir, Some(&opts))
            .map_err(|e| format!("failed to create worktree: {}", e))?;

        // Symlink .env from the main repo into the worktree so all sessions
        // share the same secrets file (and controller-cli env set updates are
        // immediately visible).
        let env_src = Path::new(repo_path).join(".env");
        let env_dst = worktree_dir.join(".env");
        #[cfg(unix)]
        if let Err(e) = std::os::unix::fs::symlink(&env_src, &env_dst) {
            eprintln!("Warning: failed to symlink .env to worktree: {}", e);
        }
        #[cfg(windows)]
        if let Err(e) = std::os::windows::fs::symlink_file(&env_src, &env_dst) {
            eprintln!("Warning: failed to symlink .env to worktree: {}", e);
        }

        Ok(worktree_dir.to_path_buf())
    }

    /// Detect the main branch name (main or master) for a repository.
    pub fn detect_main_branch(repo_path: &str) -> Result<String, String> {
        let repo = Repository::open(repo_path)
            .map_err(|e| format!("failed to open repo: {}", e))?;

        for name in &["main", "master"] {
            if repo.find_branch(name, git2::BranchType::Local).is_ok() {
                return Ok(name.to_string());
            }
        }

        // Fall back to whatever HEAD points to
        let head = repo.head().map_err(|e| format!("failed to get HEAD: {}", e))?;
        if let Some(shorthand) = head.shorthand() {
            return Ok(shorthand.to_string());
        }

        Err("Could not detect main branch".to_string())
    }

    /// Sync the main branch by pulling from remote.
    /// Runs `git pull` in the repo directory.
    pub fn sync_main(repo_path: &str) -> Result<(), String> {
        let output = Command::new("git")
            .args(["pull"])
            .current_dir(repo_path)
            .output()
            .map_err(|e| format!("failed to run git pull: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Ignore "no remote" errors — local-only repos are fine
            if stderr.contains("No remote") || stderr.contains("no tracking information") {
                return Ok(());
            }
            return Err(format!("git pull failed: {}", stderr.trim()));
        }
        Ok(())
    }

    /// Merge a session branch into main via rebase + GitHub PR.
    ///
    /// Steps:
    /// 1. Sync main (git pull)
    /// 2. Rebase session branch onto main
    /// 3. If conflicts, leave worktree in conflicted state (caller sends prompt to Claude)
    /// 4. Push branch to remote
    /// 5. Create PR via gh CLI
    pub fn merge_via_pr(
        repo_path: &str,
        worktree_path: &str,
        branch_name: &str,
    ) -> Result<MergeResult, String> {
        let main_branch = Self::detect_main_branch(repo_path)?;

        // 1. Sync main
        Self::sync_main(repo_path)?;

        // 2. Rebase session branch onto main
        let rebase_output = Command::new("git")
            .args(["rebase", &main_branch])
            .current_dir(worktree_path)
            .output()
            .map_err(|e| format!("failed to run git rebase: {}", e))?;

        if !rebase_output.status.success() {
            // Leave the rebase in progress — don't abort.
            // Caller will send a prompt to Claude in the session to resolve conflicts.
            return Ok(MergeResult::RebaseConflicts);
        }

        // 3. Push branch to remote
        Self::push_and_create_pr(worktree_path, branch_name)
    }

    /// Push branch and create a PR. Called after a clean rebase (or after
    /// Claude resolves conflicts and the user retries 'm').
    pub fn push_and_create_pr(
        worktree_path: &str,
        branch_name: &str,
    ) -> Result<MergeResult, String> {
        // Push branch to remote
        let push_output = Command::new("git")
            .args(["push", "-u", "origin", branch_name, "--force-with-lease"])
            .current_dir(worktree_path)
            .output()
            .map_err(|e| format!("failed to run git push: {}", e))?;

        if !push_output.status.success() {
            let stderr = String::from_utf8_lossy(&push_output.stderr);
            return Err(format!("Push failed: {}", stderr.trim()));
        }

        // Create PR via gh CLI
        let pr_output = Command::new("gh")
            .args(["pr", "create", "--fill", "--head", branch_name])
            .current_dir(worktree_path)
            .output()
            .map_err(|e| format!("failed to run gh pr create: {}", e))?;

        if !pr_output.status.success() {
            let stderr = String::from_utf8_lossy(&pr_output.stderr);
            // If PR already exists, try to get its URL
            if stderr.contains("already exists") {
                let view_output = Command::new("gh")
                    .args(["pr", "view", branch_name, "--json", "url", "-q", ".url"])
                    .current_dir(worktree_path)
                    .output()
                    .map_err(|e| format!("failed to get existing PR: {}", e))?;

                if view_output.status.success() {
                    let url = String::from_utf8_lossy(&view_output.stdout).trim().to_string();
                    return Ok(MergeResult::PrCreated(url));
                }
            }
            return Err(format!("PR creation failed: {}", stderr.trim()));
        }

        let pr_url = String::from_utf8_lossy(&pr_output.stdout).trim().to_string();
        Ok(MergeResult::PrCreated(pr_url))
    }

    /// Check if a worktree is in the middle of a rebase.
    pub fn is_rebase_in_progress(worktree_path: &str) -> bool {
        let git_dir = Path::new(worktree_path).join(".git");
        // Worktrees use a .git file pointing to the real git dir
        if git_dir.is_file() {
            if let Ok(content) = std::fs::read_to_string(&git_dir) {
                if let Some(real_dir) = content.strip_prefix("gitdir: ") {
                    let real_dir = real_dir.trim();
                    return Path::new(real_dir).join("rebase-merge").exists()
                        || Path::new(real_dir).join("rebase-apply").exists();
                }
            }
        }
        // Fallback: check directly
        git_dir.join("rebase-merge").exists() || git_dir.join("rebase-apply").exists()
    }

    /// Check if `branch` needs rebasing onto `main_branch` (behind or diverged).
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

        // Branch needs rebase if main has commits not in branch (behind or diverged)
        Ok(merge_base != main_commit)
    }

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

    /// Remove a worktree by deleting its directory and pruning the worktree reference.
    ///
    /// `worktree_path` is the actual directory on disk. `repo_path` is the main
    /// repository so we can prune the git reference. `branch_name` identifies the
    /// worktree within git.
    pub fn remove_worktree(
        worktree_path: &str,
        repo_path: &str,
        branch_name: &str,
    ) -> Result<(), String> {
        let worktree_dir = Path::new(worktree_path);

        // Remove the worktree directory if it exists
        if worktree_dir.exists() {
            std::fs::remove_dir_all(worktree_dir)
                .map_err(|e| format!("failed to remove worktree dir: {}", e))?;
        }

        // Prune the worktree reference
        let repo = Repository::open(repo_path)
            .map_err(|e| format!("failed to open repo: {}", e))?;

        if let Ok(wt) = repo.find_worktree(branch_name) {
            let mut prune_opts = git2::WorktreePruneOptions::new();
            prune_opts.valid(true);
            prune_opts.working_tree(true);
            wt.prune(Some(&mut prune_opts))
                .map_err(|e| format!("failed to prune worktree: {}", e))?;
        }

        // Clean up the branch so it doesn't block future worktree creation
        if let Ok(mut branch) = repo.find_branch(branch_name, git2::BranchType::Local) {
            let _ = branch.delete();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper: create a temporary git repo with an initial commit so HEAD exists.
    fn setup_test_repo() -> (TempDir, String) {
        let tmp = TempDir::new().expect("create temp dir");
        let repo_path = tmp.path().to_str().unwrap().to_string();

        let repo = Repository::init(&repo_path).expect("init repo");
        let sig = repo.signature().unwrap_or_else(|_| {
            git2::Signature::now("Test", "test@example.com").unwrap()
        });

        // Add .gitignore that excludes .env (matches real projects)
        std::fs::write(tmp.path().join(".gitignore"), ".env\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(".gitignore")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        // Create initial commit
        repo.commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
            .expect("initial commit");

        (tmp, repo_path)
    }

    #[test]
    fn test_create_and_remove_worktree() {
        let (_tmp, repo_path) = setup_test_repo();
        let wt_dir = TempDir::new().expect("create wt temp dir");
        let worktree_dir = wt_dir.path().join("feature-test");

        // Create a worktree
        let wt_path = WorktreeManager::create_worktree(&repo_path, "feature-test", &worktree_dir)
            .expect("create worktree");

        // Verify the worktree directory exists and has a .git marker
        assert!(wt_path.exists(), "worktree directory should exist");
        assert!(
            wt_path.join(".git").exists(),
            "worktree should have a .git file"
        );

        // Remove the worktree
        WorktreeManager::remove_worktree(
            wt_path.to_str().unwrap(),
            &repo_path,
            "feature-test",
        )
        .expect("remove worktree");

        // Verify the directory is gone
        assert!(!wt_path.exists(), "worktree directory should be removed");
    }

    #[test]
    fn test_duplicate_worktree_fails() {
        let (_tmp, repo_path) = setup_test_repo();
        let wt_dir = TempDir::new().expect("create wt temp dir");
        let worktree_dir = wt_dir.path().join("dupe-branch");

        // Create first worktree
        WorktreeManager::create_worktree(&repo_path, "dupe-branch", &worktree_dir)
            .expect("first create should succeed");

        // Try to create another with the same name - should fail
        let result = WorktreeManager::create_worktree(&repo_path, "dupe-branch", &worktree_dir);
        assert!(result.is_err(), "duplicate worktree should fail");
        assert!(
            result.unwrap_err().contains("already exists"),
            "error should mention 'already exists'"
        );
    }

    #[test]
    fn test_detect_main_branch() {
        let (_tmp, repo_path) = setup_test_repo();
        // Default branch from init + commit on HEAD is typically "main" or "master"
        let branch = WorktreeManager::detect_main_branch(&repo_path).expect("detect main branch");
        assert!(
            branch == "main" || branch == "master",
            "expected 'main' or 'master', got '{}'",
            branch
        );
    }

    #[test]
    fn test_sync_main_local_only_repo() {
        let (_tmp, repo_path) = setup_test_repo();
        // sync_main on a repo with no remote should succeed (no-op)
        let result = WorktreeManager::sync_main(&repo_path);
        assert!(result.is_ok(), "sync_main should succeed on local-only repo: {:?}", result);
    }

    #[test]
    fn test_unborn_branch_returns_sentinel_error() {
        let tmp = TempDir::new().expect("create temp dir");
        let repo_path = tmp.path().to_str().unwrap().to_string();

        // Init repo but make NO commits — HEAD is unborn
        Repository::init(&repo_path).expect("init repo");

        let wt_dir = TempDir::new().expect("create wt temp dir");
        let worktree_dir = wt_dir.path().join("session-1");

        let result = WorktreeManager::create_worktree(&repo_path, "session-1", &worktree_dir);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "unborn_branch");
    }

    #[test]
    fn test_is_rebase_in_progress_false_for_regular_dir() {
        let tmp = TempDir::new().expect("create temp dir");
        assert!(!WorktreeManager::is_rebase_in_progress(
            tmp.path().to_str().unwrap()
        ));
    }

    #[test]
    fn test_is_rebase_in_progress_false_for_clean_repo() {
        let (_tmp, repo_path) = setup_test_repo();
        assert!(!WorktreeManager::is_rebase_in_progress(&repo_path));
    }

    #[test]
    fn test_remove_worktree_nonexistent_path_prunes_reference() {
        let (_tmp, repo_path) = setup_test_repo();
        // Removing a worktree that doesn't exist on disk should not error
        let result = WorktreeManager::remove_worktree(
            "/tmp/nonexistent-worktree-path",
            &repo_path,
            "nonexistent-branch",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_worktree_symlinks_env_file() {
        let (_tmp, repo_path) = setup_test_repo();

        let env_content = "SECRET_KEY=abc123\nDB_URL=postgres://localhost/test\n";
        std::fs::write(Path::new(&repo_path).join(".env"), env_content).unwrap();

        let wt_dir = TempDir::new().expect("create wt temp dir");
        let worktree_dir = wt_dir.path().join("env-test");

        let wt_path = WorktreeManager::create_worktree(&repo_path, "env-test", &worktree_dir)
            .expect("create worktree");

        let wt_env = wt_path.join(".env");
        assert!(wt_env.is_symlink(), ".env should be a symlink");
        assert_eq!(
            std::fs::read_to_string(&wt_env).unwrap(),
            env_content,
            ".env contents should match via symlink"
        );

        // Updating the source .env should be visible through the symlink
        let updated = "SECRET_KEY=updated\n";
        std::fs::write(Path::new(&repo_path).join(".env"), updated).unwrap();
        assert_eq!(
            std::fs::read_to_string(&wt_env).unwrap(),
            updated,
            "worktree should see updated .env"
        );
    }

    #[test]
    fn test_create_worktree_symlinks_env_even_when_missing() {
        let (_tmp, repo_path) = setup_test_repo();
        // No .env in repo yet
        let wt_dir = TempDir::new().expect("create wt temp dir");
        let worktree_dir = wt_dir.path().join("no-env-test");

        let wt_path = WorktreeManager::create_worktree(&repo_path, "no-env-test", &worktree_dir)
            .expect("create worktree");

        let wt_env = wt_path.join(".env");
        // Symlink exists but target doesn't — that's fine
        assert!(wt_env.is_symlink(), ".env symlink should exist");
        assert!(!wt_env.exists(), "symlink target should not exist yet");

        // Creating .env in the repo makes it visible through the symlink
        let content = "NEW_KEY=value\n";
        std::fs::write(Path::new(&repo_path).join(".env"), content).unwrap();
        assert_eq!(
            std::fs::read_to_string(&wt_env).unwrap(),
            content,
            "worktree should see newly created .env"
        );
    }

    #[test]
    fn test_detect_main_branch_on_empty_repo_errors() {
        let tmp = TempDir::new().expect("create temp dir");
        let repo_path = tmp.path().to_str().unwrap().to_string();
        Repository::init(&repo_path).expect("init repo");
        // No commits = unborn HEAD, detect_main_branch should error
        let result = WorktreeManager::detect_main_branch(&repo_path);
        assert!(result.is_err());
    }

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

}
