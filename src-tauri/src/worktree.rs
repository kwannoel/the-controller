use git2::Repository;
use std::path::{Path, PathBuf};

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
