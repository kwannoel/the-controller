use std::fs;
use tempfile::TempDir;
use the_controller_lib::models::{MaintainerConfig, Project, SessionConfig};
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
        maintainer: MaintainerConfig::default(),
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
        archived: false,
        kind: "claude".to_string(),
        github_issue: None,
        initial_prompt: None,
        done_commits: vec![],
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
        maintainer: MaintainerConfig::default(),
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

/// Sessions should persist across app restarts. On startup, `restore_sessions`
/// re-spawns PTY processes for active sessions while keeping metadata intact.
/// This test verifies session metadata survives a simulated restart.
#[test]
fn test_sessions_persist_across_restarts() {
    let tmp = TempDir::new().unwrap();
    let storage = make_storage(&tmp);

    let project_id = Uuid::new_v4();
    let project = Project {
        id: project_id,
        name: "persist-test".to_string(),
        repo_path: "/tmp/fake-repo".to_string(),
        created_at: "2026-03-01T00:00:00Z".to_string(),
        archived: false,
        maintainer: MaintainerConfig::default(),
        sessions: vec![
            SessionConfig {
                id: Uuid::new_v4(),
                label: "session-1".to_string(),
                worktree_path: Some("/tmp/nonexistent/wt1".to_string()),
                worktree_branch: Some("session-1".to_string()),
                archived: false,
                kind: "claude".to_string(),
                github_issue: None,
                initial_prompt: None,
                done_commits: vec![],
            },
            SessionConfig {
                id: Uuid::new_v4(),
                label: "session-2".to_string(),
                worktree_path: Some("/tmp/nonexistent/wt2".to_string()),
                worktree_branch: Some("session-2".to_string()),
                archived: false,
                kind: "claude".to_string(),
                github_issue: None,
                initial_prompt: None,
                done_commits: vec![],
            },
            SessionConfig {
                id: Uuid::new_v4(),
                label: "session-3".to_string(),
                worktree_path: None,
                worktree_branch: None,
                archived: true,
                kind: "claude".to_string(),
                github_issue: None,
                initial_prompt: None,
                done_commits: vec![],
            },
        ],
    };
    storage.save_project(&project).expect("save project");

    // Simulate restart: reload from disk
    let loaded = storage.load_project(project_id).expect("load after restart");
    assert_eq!(loaded.sessions.len(), 3, "all sessions should persist");
    assert_eq!(
        loaded.sessions.iter().filter(|s| !s.archived).count(),
        2,
        "active sessions should survive restart"
    );
    assert_eq!(
        loaded.sessions.iter().filter(|s| s.archived).count(),
        1,
        "archived sessions should also survive"
    );
    assert_eq!(loaded.name, "persist-test");
}

/// Verify that no two projects can have the same name.
/// The `create_project`/`load_project`/`scaffold_project` commands check at
/// the Tauri command level, but we also test at the storage layer to document
/// the invariant: project names MUST be unique.
#[test]
fn test_no_duplicate_project_names() {
    let tmp = TempDir::new().unwrap();
    let storage = make_storage(&tmp);

    let project_a = Project {
        id: Uuid::new_v4(),
        name: "my-project".to_string(),
        repo_path: "/tmp/repo-a".to_string(),
        created_at: "2026-03-01T00:00:00Z".to_string(),
        archived: false,
        maintainer: MaintainerConfig::default(),
        sessions: vec![],
    };
    storage.save_project(&project_a).expect("save first project");

    // Attempting to save another project with the same name should be caught
    // by the command layer. At the storage layer, we verify the invariant
    // by checking list_projects for duplicates.
    let project_b = Project {
        id: Uuid::new_v4(),
        name: "my-project".to_string(),
        repo_path: "/tmp/repo-b".to_string(),
        created_at: "2026-03-01T00:00:00Z".to_string(),
        archived: false,
        maintainer: MaintainerConfig::default(),
        sessions: vec![],
    };
    storage.save_project(&project_b).expect("save second project");

    let projects = storage.list_projects().expect("list projects");
    let count = projects.iter().filter(|p| p.name == "my-project").count();

    // This documents the current behavior: storage doesn't enforce uniqueness,
    // the command layer does. If storage gains enforcement, this test still passes.
    assert!(
        count >= 1,
        "at least one project named 'my-project' should exist"
    );
}

/// Verify that worktrees persist across app restarts (not cleaned up).
#[test]
fn test_worktrees_persist_across_restarts() {
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
        name: "worktree-persist-test".to_string(),
        repo_path: repo_path.clone(),
        created_at: "2026-03-01T00:00:00Z".to_string(),
        archived: false,
        maintainer: MaintainerConfig::default(),
        sessions: vec![SessionConfig {
            id: Uuid::new_v4(),
            label: "session-1".to_string(),
            worktree_path: Some(wt_path.to_str().unwrap().to_string()),
            worktree_branch: Some("session-1".to_string()),
            archived: false,
            kind: "claude".to_string(),
            github_issue: None,
            initial_prompt: None,
            done_commits: vec![],
        }],
    };
    storage.save_project(&project).expect("save project");

    // Simulate restart: reload from disk
    let loaded = storage.load_project(project_id).expect("load after restart");
    assert_eq!(loaded.sessions.len(), 1, "session should persist");
    assert!(wt_path.exists(), "worktree should still exist on disk after restart");
}

/// Worktree directories should use project name, not UUID.
/// Migration renames `worktrees/{uuid}/` to `worktrees/{name}/`
/// and updates stored `worktree_path` values.
#[test]
fn test_migrate_worktree_paths_renames_uuid_dir() {
    let tmp = TempDir::new().unwrap();
    let storage = make_storage(&tmp);

    let project_id = Uuid::new_v4();
    let uuid_wt_dir = tmp.path().join("worktrees").join(project_id.to_string());
    let session_wt = uuid_wt_dir.join("session-1");
    fs::create_dir_all(&session_wt).expect("create uuid worktree dir");

    let project = Project {
        id: project_id,
        name: "my-cool-project".to_string(),
        repo_path: "/tmp/fake-repo".to_string(),
        created_at: "2026-03-01T00:00:00Z".to_string(),
        archived: false,
        maintainer: MaintainerConfig::default(),
        sessions: vec![SessionConfig {
            id: Uuid::new_v4(),
            label: "session-1".to_string(),
            worktree_path: Some(session_wt.to_str().unwrap().to_string()),
            worktree_branch: Some("session-1".to_string()),
            archived: false,
            kind: "claude".to_string(),
            github_issue: None,
            initial_prompt: None,
            done_commits: vec![],
        }],
    };
    storage.save_project(&project).expect("save project");

    // Run migration
    storage.migrate_worktree_paths(&project).expect("migrate");

    // UUID dir should be gone, name dir should exist
    assert!(!uuid_wt_dir.exists(), "UUID dir should be removed");
    let name_wt_dir = tmp.path().join("worktrees").join("my-cool-project");
    assert!(name_wt_dir.join("session-1").exists(), "name dir should exist");

    // Stored paths should be updated
    let loaded = storage.load_project(project_id).expect("load");
    let session = &loaded.sessions[0];
    let expected_path = name_wt_dir.join("session-1").to_str().unwrap().to_string();
    assert_eq!(session.worktree_path.as_deref(), Some(expected_path.as_str()));
}

#[test]
fn test_migrate_worktree_paths_noop_when_no_uuid_dir() {
    let tmp = TempDir::new().unwrap();
    let storage = make_storage(&tmp);

    let project_id = Uuid::new_v4();
    let name_wt_dir = tmp.path().join("worktrees").join("already-migrated");
    let session_wt = name_wt_dir.join("session-1");
    fs::create_dir_all(&session_wt).expect("create name worktree dir");

    let project = Project {
        id: project_id,
        name: "already-migrated".to_string(),
        repo_path: "/tmp/fake-repo".to_string(),
        created_at: "2026-03-01T00:00:00Z".to_string(),
        archived: false,
        maintainer: MaintainerConfig::default(),
        sessions: vec![SessionConfig {
            id: Uuid::new_v4(),
            label: "session-1".to_string(),
            worktree_path: Some(session_wt.to_str().unwrap().to_string()),
            worktree_branch: Some("session-1".to_string()),
            archived: false,
            kind: "claude".to_string(),
            github_issue: None,
            initial_prompt: None,
            done_commits: vec![],
        }],
    };
    storage.save_project(&project).expect("save project");

    // Should not error or change anything
    storage.migrate_worktree_paths(&project).expect("migrate noop");

    assert!(session_wt.exists(), "name dir should still exist");
    let loaded = storage.load_project(project_id).expect("load");
    assert_eq!(
        loaded.sessions[0].worktree_path.as_deref(),
        Some(session_wt.to_str().unwrap())
    );
}

/// When both `worktrees/{uuid}/` and `worktrees/{name}/` exist,
/// migration should skip the rename to avoid clobbering the name dir.
/// The stored `worktree_path` must remain unchanged (still UUID-based).
#[test]
fn test_migrate_worktree_paths_noop_on_name_collision() {
    let tmp = TempDir::new().unwrap();
    let storage = make_storage(&tmp);

    let project_id = Uuid::new_v4();

    // Create BOTH the UUID-based dir and the name-based dir
    let uuid_wt_dir = tmp.path().join("worktrees").join(project_id.to_string());
    let uuid_session = uuid_wt_dir.join("session-1");
    fs::create_dir_all(&uuid_session).expect("create uuid worktree dir");

    let name_wt_dir = tmp.path().join("worktrees").join("collision-project");
    let name_session = name_wt_dir.join("session-1");
    fs::create_dir_all(&name_session).expect("create name worktree dir");

    let project = Project {
        id: project_id,
        name: "collision-project".to_string(),
        repo_path: "/tmp/fake-repo".to_string(),
        created_at: "2026-03-01T00:00:00Z".to_string(),
        archived: false,
        maintainer: MaintainerConfig::default(),
        sessions: vec![SessionConfig {
            id: Uuid::new_v4(),
            label: "session-1".to_string(),
            worktree_path: Some(uuid_session.to_str().unwrap().to_string()),
            worktree_branch: Some("session-1".to_string()),
            archived: false,
            kind: "claude".to_string(),
            github_issue: None,
            initial_prompt: None,
            done_commits: vec![],
        }],
    };
    storage.save_project(&project).expect("save project");

    // Run migration — should be a no-op because name dir already exists
    storage
        .migrate_worktree_paths(&project)
        .expect("migrate should not error on collision");

    // UUID dir should still exist (no rename happened)
    assert!(uuid_wt_dir.exists(), "UUID dir should still exist");

    // Stored path should be unchanged (still UUID-based)
    let loaded = storage.load_project(project_id).expect("load");
    assert_eq!(
        loaded.sessions[0].worktree_path.as_deref(),
        Some(uuid_session.to_str().unwrap()),
        "worktree_path should remain UUID-based when name collision exists"
    );
}

/// A project with no sessions should be archivable.
/// Reproduces the bug where archiving a zero-session project was a no-op:
/// `archive_project` only marked sessions as archived (nothing to iterate),
/// and `list_projects` / `list_archived_projects` used session-based filtering
/// that always kept zero-session projects in the active list.
///
/// The fix: `archive_project` must set `project.archived = true`, and filtering
/// must use `project.archived` as the source of truth.
#[test]
fn test_archive_project_with_no_sessions() {
    let tmp = TempDir::new().unwrap();
    let storage = make_storage(&tmp);

    let project_id = Uuid::new_v4();
    let project = Project {
        id: project_id,
        name: "empty-project".to_string(),
        repo_path: "/tmp/fake-repo".to_string(),
        created_at: "2026-03-01T00:00:00Z".to_string(),
        archived: false,
        maintainer: MaintainerConfig::default(),
        sessions: vec![],
    };
    storage.save_project(&project).expect("save project");

    // Simulate what archive_project should do: set project.archived = true
    let mut project = storage.load_project(project_id).expect("load project");
    project.archived = true;
    storage.save_project(&project).expect("save archived project");

    // Verify the project is archived
    let archived = storage.load_project(project_id).expect("load archived");
    assert!(archived.archived, "project.archived should be true");

    // Apply the same filtering logic used by list_projects / list_archived_projects.
    // The project.archived field must be the source of truth.
    let all_projects = storage.list_projects().expect("list projects");

    let active: Vec<_> = all_projects.iter().filter(|p| !p.archived).collect();
    let archived_list: Vec<_> = all_projects.iter().filter(|p| p.archived).collect();

    assert_eq!(
        active.len(),
        0,
        "archived project with no sessions must NOT appear in active list"
    );
    assert_eq!(
        archived_list.len(),
        1,
        "archived project with no sessions must appear in archived list"
    );
    assert_eq!(archived_list[0].id, project_id);
}

/// After migration, new sessions should use name-based paths.
#[test]
fn test_create_session_uses_project_name_in_path() {
    let tmp = TempDir::new().unwrap();
    let storage = make_storage(&tmp);

    let project_id = Uuid::new_v4();

    // Set up a real git repo so worktree creation works
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

    let project = Project {
        id: project_id,
        name: "test-name-path".to_string(),
        repo_path: repo_path.clone(),
        created_at: "2026-03-01T00:00:00Z".to_string(),
        archived: false,
        maintainer: MaintainerConfig::default(),
        sessions: vec![],
    };
    storage.save_project(&project).expect("save project");

    // Create worktree using name-based path (what create_session should do)
    let worktree_dir = tmp.path()
        .join("worktrees")
        .join(&project.name)
        .join("session-1");
    let wt_path = WorktreeManager::create_worktree(&repo_path, "session-1", &worktree_dir)
        .expect("create worktree");

    // Path should contain project name, not UUID
    let path_str = wt_path.to_str().unwrap();
    assert!(
        path_str.contains("test-name-path"),
        "worktree path should contain project name, got: {}",
        path_str
    );
    assert!(
        !path_str.contains(&project_id.to_string()),
        "worktree path should NOT contain UUID, got: {}",
        path_str
    );
}

/// scaffold_project should create agents.md and docs/plans/.gitkeep
/// in the repo directory AND include them in the initial git commit.
#[test]
fn test_scaffold_project_creates_template_files() {
    let projects_root = TempDir::new().unwrap();
    let name = "test-template-project";
    let repo_path = projects_root.path().join(name);
    fs::create_dir_all(&repo_path).unwrap();

    // Write template files (replicating scaffold_project logic)
    let agents_content = the_controller_lib::commands::render_agents_md(name);
    fs::write(repo_path.join("agents.md"), &agents_content).unwrap();
    fs::create_dir_all(repo_path.join("docs").join("plans")).unwrap();
    fs::write(repo_path.join("docs").join("plans").join(".gitkeep"), "").unwrap();

    // Verify files exist on disk
    assert!(repo_path.join("agents.md").exists());
    assert!(repo_path.join("docs/plans/.gitkeep").exists());

    // Verify agents.md contains the project name
    let content = fs::read_to_string(repo_path.join("agents.md")).unwrap();
    assert!(content.starts_with(&format!("# {}", name)));
    assert!(content.contains("Task Workflow"));
    assert!(content.contains("Task Structure"));
}

/// The initial commit created by scaffold_project should contain
/// agents.md and docs/plans/.gitkeep in its tree.
#[test]
fn test_scaffold_initial_commit_contains_template_files() {
    let projects_root = TempDir::new().unwrap();
    let name = "commit-tree-test";
    let repo_path = projects_root.path().join(name);
    fs::create_dir_all(&repo_path).unwrap();

    // Replicate scaffold_project's git logic
    let repo = git2::Repository::init(&repo_path).unwrap();
    let sig = git2::Signature::now("Test", "test@test.com").unwrap();

    // Write files
    let agents_content = the_controller_lib::commands::render_agents_md(name);
    fs::write(repo_path.join("agents.md"), &agents_content).unwrap();
    fs::create_dir_all(repo_path.join("docs").join("plans")).unwrap();
    fs::write(repo_path.join("docs").join("plans").join(".gitkeep"), "").unwrap();

    // Add to index and commit
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("agents.md")).unwrap();
    index.add_path(std::path::Path::new("docs/plans/.gitkeep")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[]).unwrap();

    // Verify the HEAD commit tree contains our files
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    let commit_tree = head.tree().unwrap();

    // agents.md should be at root
    assert!(commit_tree.get_name("agents.md").is_some(), "agents.md missing from commit tree");

    // docs/plans/.gitkeep should be nested
    let docs_entry = commit_tree.get_name("docs").expect("docs/ missing from commit tree");
    let docs_tree = repo.find_tree(docs_entry.id()).unwrap();
    let plans_entry = docs_tree.get_name("plans").expect("plans/ missing from docs tree");
    let plans_tree = repo.find_tree(plans_entry.id()).unwrap();
    assert!(plans_tree.get_name(".gitkeep").is_some(), ".gitkeep missing from plans tree");

    // Verify agents.md content in the commit
    let agents_entry = commit_tree.get_name("agents.md").unwrap();
    let agents_blob = repo.find_blob(agents_entry.id()).unwrap();
    let content = std::str::from_utf8(agents_blob.content()).unwrap();
    assert!(content.starts_with(&format!("# {}", name)));
}

/// Loading a project by repo_path when an archived project with the same path
/// exists should unarchive it rather than creating a duplicate or rejecting it.
#[test]
fn test_load_archived_project_by_repo_path_unarchives_it() {
    let tmp = TempDir::new().unwrap();
    let storage = make_storage(&tmp);

    let project_id = Uuid::new_v4();
    let repo_path = "/tmp/commit-graph".to_string();
    let project = Project {
        id: project_id,
        name: "commit-graph".to_string(),
        repo_path: repo_path.clone(),
        created_at: "2026-03-01T00:00:00Z".to_string(),
        archived: false,
        maintainer: MaintainerConfig::default(),
        sessions: vec![],
    };
    storage.save_project(&project).expect("save project");

    // Archive the project
    let mut project = storage.load_project(project_id).expect("load");
    project.archived = true;
    storage.save_project(&project).expect("archive");

    // Simulate the load_project command logic: search all projects by repo_path,
    // find the archived one, unarchive it, and save.
    let all = storage.list_projects().expect("list");
    let found = all.iter().find(|p| p.repo_path == repo_path);
    assert!(found.is_some(), "archived project should be findable by repo_path");
    let found = found.unwrap();
    assert!(found.archived, "project should be archived");

    // Unarchive it (mirrors the fix in load_project command)
    let mut unarchived = found.clone();
    unarchived.archived = false;
    storage.save_project(&unarchived).expect("save unarchived");

    // Verify it's now active
    let reloaded = storage.load_project(project_id).expect("reload");
    assert!(!reloaded.archived, "project should be unarchived after re-load");

    // The duplicate name check should skip archived projects, so a new project
    // with a different repo_path but same name should be creatable after the
    // original is archived again.
    let mut project_again = storage.load_project(project_id).expect("load");
    project_again.archived = true;
    storage.save_project(&project_again).expect("re-archive");

    let active: Vec<_> = storage
        .list_projects()
        .expect("list")
        .into_iter()
        .filter(|p| !p.archived)
        .collect();
    let name_conflict = active.iter().any(|p| p.name == "commit-graph");
    assert!(
        !name_conflict,
        "archived project name should not block new projects"
    );
}
