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

        // Write an empty tree
        let tree_id = repo.treebuilder(None).unwrap().write().unwrap();
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
}
