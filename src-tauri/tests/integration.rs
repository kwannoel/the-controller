use std::fs;
use tempfile::TempDir;
use the_controller_lib::models::{Project, SessionConfig};
use the_controller_lib::storage::Storage;
use the_controller_lib::worktree::WorktreeManager;
use uuid::Uuid;

fn make_storage(tmp: &TempDir) -> Storage {
    let storage = Storage::new(tmp.path().to_path_buf());
    storage.ensure_dirs().expect("ensure_dirs");
    storage
}

#[test]
fn test_project_lifecycle() {
    let tmp = TempDir::new().unwrap();
    let storage = make_storage(&tmp);

    // Create and save a project
    let project = Project {
        id: Uuid::new_v4(),
        name: "lifecycle-test".to_string(),
        repo_path: "/tmp/fake-repo".to_string(),
        created_at: "2026-03-01T00:00:00Z".to_string(),
        archived: false,
        sessions: vec![],
    };
    storage.save_project(&project).expect("save project");

    // Add a session and save again
    let mut project = storage.load_project(project.id).expect("load after save");
    assert_eq!(project.sessions.len(), 0);

    project.sessions.push(SessionConfig {
        id: Uuid::new_v4(),
        label: "session-1".to_string(),
        worktree_path: None,
        worktree_branch: None,
    });
    storage.save_project(&project).expect("save with session");

    // List projects — should contain exactly one
    let projects = storage.list_projects().expect("list projects");
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].sessions.len(), 1);

    // Archive the project
    let mut project = storage.load_project(project.id).expect("load for archive");
    project.archived = true;
    project.sessions.clear();
    storage.save_project(&project).expect("save archived");

    // Verify archived state
    let archived = storage.load_project(project.id).expect("load archived");
    assert!(archived.archived);
    assert!(archived.sessions.is_empty());
}

#[test]
fn test_agents_md_lifecycle() {
    let tmp = TempDir::new().unwrap();
    let repo_dir = TempDir::new().unwrap();
    let storage = make_storage(&tmp);

    let project = Project {
        id: Uuid::new_v4(),
        name: "agents-test".to_string(),
        repo_path: repo_dir.path().to_str().unwrap().to_string(),
        created_at: "2026-03-01T00:00:00Z".to_string(),
        archived: false,
        sessions: vec![],
    };
    storage.save_project(&project).expect("save project");

    // Save agents.md via storage (config dir)
    let content = "# Test Agents\n\nCustom instructions here.\n";
    storage
        .save_agents_md(project.id, content)
        .expect("save agents.md");

    // Read it back — should return config-dir content
    let read_back = storage.get_agents_md(&project).expect("read agents.md");
    assert_eq!(read_back, content);

    // Write agents.md in the repo root — this should take priority
    let repo_content = "# Repo-Level Agents\n\nHigher priority content.\n";
    fs::write(repo_dir.path().join("agents.md"), repo_content)
        .expect("write repo agents.md");

    // Verify repo-root file takes priority
    let priority_read = storage.get_agents_md(&project).expect("read priority agents.md");
    assert_eq!(priority_read, repo_content);
}

/// Simulate the stale-sessions bug: sessions persisted from a previous app run
/// should be cleared on startup so they don't appear as ghost entries.
///
/// This test reproduces the bug by:
/// 1. Creating a project with sessions saved to disk (simulating a previous run)
/// 2. Calling the public `cleanup_stale_sessions` helper
/// 3. Verifying all sessions are cleared
///
/// Without the cleanup, the stale sessions persist and accumulate,
/// making it look like pressing 'c' creates multiple sessions at once.
#[test]
fn test_stale_sessions_cleared_on_startup() {
    let tmp = TempDir::new().unwrap();
    let storage = make_storage(&tmp);

    // Simulate a previous app run that left sessions persisted on disk
    let project_id = Uuid::new_v4();
    let project = Project {
        id: project_id,
        name: "stale-test".to_string(),
        repo_path: "/tmp/fake-repo".to_string(),
        created_at: "2026-03-01T00:00:00Z".to_string(),
        archived: false,
        sessions: vec![
            SessionConfig {
                id: Uuid::new_v4(),
                label: "session-1".to_string(),
                worktree_path: Some("/tmp/nonexistent/wt1".to_string()),
                worktree_branch: Some("session-1".to_string()),
            },
            SessionConfig {
                id: Uuid::new_v4(),
                label: "session-2".to_string(),
                worktree_path: Some("/tmp/nonexistent/wt2".to_string()),
                worktree_branch: Some("session-2".to_string()),
            },
            SessionConfig {
                id: Uuid::new_v4(),
                label: "session-3".to_string(),
                worktree_path: Some("/tmp/nonexistent/wt3".to_string()),
                worktree_branch: Some("session-3".to_string()),
            },
        ],
    };
    storage.save_project(&project).expect("save project with stale sessions");

    // Verify the stale sessions are persisted
    let loaded = storage.load_project(project_id).expect("load before cleanup");
    assert_eq!(loaded.sessions.len(), 3, "stale sessions should be persisted on disk");

    // --- Simulate app startup: call the cleanup function ---
    the_controller_lib::commands::do_cleanup_stale_sessions(&storage);

    // Verify sessions are cleared after cleanup
    let cleaned = storage.load_project(project_id).expect("load after cleanup");
    assert_eq!(
        cleaned.sessions.len(),
        0,
        "stale sessions must be cleared on startup — without cleanup, \
         ghost sessions accumulate and pressing 'c' appears to create multiple sessions"
    );

    // Project metadata should still exist
    assert_eq!(cleaned.name, "stale-test");
    assert!(!cleaned.archived);
}

/// Verify that cleanup also works correctly with a real worktree on disk.
#[test]
fn test_stale_sessions_cleanup_removes_worktrees() {
    let tmp = TempDir::new().unwrap();
    let storage = make_storage(&tmp);

    // Set up a real git repo
    let repo_dir = TempDir::new().unwrap();
    let repo_path = repo_dir.path().to_str().unwrap().to_string();
    let repo = git2::Repository::init(&repo_path).expect("init repo");
    let sig = repo.signature().unwrap_or_else(|_| {
        git2::Signature::now("Test", "test@example.com").unwrap()
    });
    let tree_id = repo.treebuilder(None).unwrap().write().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
        .expect("initial commit");

    // Create a real worktree
    let wt_dir = tmp.path().join("worktrees").join("session-1");
    let wt_path = WorktreeManager::create_worktree(&repo_path, "session-1", &wt_dir)
        .expect("create worktree");
    assert!(wt_path.exists(), "worktree should exist on disk");

    // Save project with session pointing to the worktree
    let project_id = Uuid::new_v4();
    let project = Project {
        id: project_id,
        name: "worktree-cleanup-test".to_string(),
        repo_path: repo_path.clone(),
        created_at: "2026-03-01T00:00:00Z".to_string(),
        archived: false,
        sessions: vec![SessionConfig {
            id: Uuid::new_v4(),
            label: "session-1".to_string(),
            worktree_path: Some(wt_path.to_str().unwrap().to_string()),
            worktree_branch: Some("session-1".to_string()),
        }],
    };
    storage.save_project(&project).expect("save project");

    // --- Call the cleanup function ---
    the_controller_lib::commands::do_cleanup_stale_sessions(&storage);

    // Verify sessions cleared and worktree removed
    let cleaned = storage.load_project(project_id).expect("load after cleanup");
    assert_eq!(cleaned.sessions.len(), 0);
    assert!(!wt_path.exists(), "worktree directory should be removed after cleanup");
}
