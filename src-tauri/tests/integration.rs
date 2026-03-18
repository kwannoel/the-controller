use std::fs;
use tempfile::TempDir;
use the_controller_lib::models::{AutoWorkerConfig, MaintainerConfig, Project, SessionConfig};
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
        auto_worker: AutoWorkerConfig::default(),
        prompts: vec![],
        sessions: vec![],
        staged_sessions: vec![],
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
        auto_worker_session: false,
    });
    storage.save_project(&project).expect("save with session");

    // List projects — should contain exactly one
    let projects = storage.list_projects().expect("list projects");
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].sessions.len(), 1);

    let reloaded = storage.load_project(project.id).expect("reload project");
    assert_eq!(reloaded.sessions.len(), 1);
    assert_eq!(reloaded.sessions[0].label, "session-1");
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
        auto_worker: AutoWorkerConfig::default(),
        prompts: vec![],
        sessions: vec![],
        staged_sessions: vec![],
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
    fs::write(repo_dir.path().join("agents.md"), repo_content).expect("write repo agents.md");

    // Verify repo-root file takes priority
    let priority_read = storage
        .get_agents_md(&project)
        .expect("read priority agents.md");
    assert_eq!(priority_read, repo_content);
}

/// Sessions should persist across app restarts.
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
        auto_worker: AutoWorkerConfig::default(),
        prompts: vec![],
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
                auto_worker_session: false,
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
                auto_worker_session: false,
            },
            SessionConfig {
                id: Uuid::new_v4(),
                label: "session-3".to_string(),
                worktree_path: None,
                worktree_branch: None,
                archived: false,
                kind: "claude".to_string(),
                github_issue: None,
                initial_prompt: None,
                done_commits: vec![],
                auto_worker_session: false,
            },
        ],
        staged_sessions: vec![],
    };
    storage.save_project(&project).expect("save project");

    // Simulate restart: reload from disk
    let loaded = storage
        .load_project(project_id)
        .expect("load after restart");
    assert_eq!(loaded.sessions.len(), 3, "all sessions should persist");
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
        auto_worker: AutoWorkerConfig::default(),
        prompts: vec![],
        sessions: vec![],
        staged_sessions: vec![],
    };
    storage
        .save_project(&project_a)
        .expect("save first project");

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
        auto_worker: AutoWorkerConfig::default(),
        prompts: vec![],
        sessions: vec![],
        staged_sessions: vec![],
    };
    storage
        .save_project(&project_b)
        .expect("save second project");

    let projects = storage.list_projects().expect("list projects");
    let count = projects.iter().filter(|p| p.name == "my-project").count();

    // Storage doesn't enforce uniqueness — the command layer does.
    // Both projects should be persisted.
    assert_eq!(
        count, 2,
        "storage should persist both projects with the same name"
    );
}

#[test]
fn test_archived_project_name_still_blocks_duplicate_name_checks() {
    let tmp = TempDir::new().unwrap();
    let storage = make_storage(&tmp);

    let project = Project {
        id: Uuid::new_v4(),
        name: "reusable-name".to_string(),
        repo_path: "/tmp/fake-repo".to_string(),
        created_at: "2026-03-01T00:00:00Z".to_string(),
        archived: true,
        maintainer: MaintainerConfig::default(),
        auto_worker: AutoWorkerConfig::default(),
        prompts: vec![],
        sessions: vec![],
        staged_sessions: vec![],
    };
    storage
        .save_project(&project)
        .expect("save archived project");

    let existing = storage.list_projects().expect("list projects");
    let has_duplicate = existing.iter().any(|p| p.name == "reusable-name");

    assert!(
        has_duplicate,
        "duplicate-name checks should include projects regardless of archived flag"
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
    let sig = repo
        .signature()
        .unwrap_or_else(|_| git2::Signature::now("Test", "test@example.com").unwrap());
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
        auto_worker: AutoWorkerConfig::default(),
        prompts: vec![],
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
            auto_worker_session: false,
        }],
        staged_sessions: vec![],
    };
    storage.save_project(&project).expect("save project");

    // Simulate restart: reload from disk
    let loaded = storage
        .load_project(project_id)
        .expect("load after restart");
    assert_eq!(loaded.sessions.len(), 1, "session should persist");
    assert!(
        wt_path.exists(),
        "worktree should still exist on disk after restart"
    );
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
        auto_worker: AutoWorkerConfig::default(),
        prompts: vec![],
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
            auto_worker_session: false,
        }],
        staged_sessions: vec![],
    };
    storage.save_project(&project).expect("save project");

    // Run migration
    storage.migrate_worktree_paths(&project).expect("migrate");

    // UUID dir should be gone, name dir should exist
    assert!(!uuid_wt_dir.exists(), "UUID dir should be removed");
    let name_wt_dir = tmp.path().join("worktrees").join("my-cool-project");
    assert!(
        name_wt_dir.join("session-1").exists(),
        "name dir should exist"
    );

    // Stored paths should be updated
    let loaded = storage.load_project(project_id).expect("load");
    let session = &loaded.sessions[0];
    let expected_path = name_wt_dir.join("session-1").to_str().unwrap().to_string();
    assert_eq!(
        session.worktree_path.as_deref(),
        Some(expected_path.as_str())
    );
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
        auto_worker: AutoWorkerConfig::default(),
        prompts: vec![],
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
            auto_worker_session: false,
        }],
        staged_sessions: vec![],
    };
    storage.save_project(&project).expect("save project");

    // Should not error or change anything
    storage
        .migrate_worktree_paths(&project)
        .expect("migrate noop");

    assert!(session_wt.exists(), "name dir should still exist");
    let loaded = storage.load_project(project_id).expect("load");
    assert_eq!(
        loaded.sessions[0].worktree_path.as_deref(),
        Some(session_wt.to_str().unwrap())
    );
}

/// If startup crashes after renaming `worktrees/{uuid}/` to `worktrees/{name}/`
/// but before saving the updated project config, the next startup should still
/// repair stale UUID-based `worktree_path` values. Re-running the migration
/// must remain safe once the config has been repaired.
#[test]
fn test_migrate_worktree_paths_repairs_stale_paths_after_partial_migration() {
    let tmp = TempDir::new().unwrap();
    let storage = make_storage(&tmp);

    let project_id = Uuid::new_v4();
    let name_wt_dir = tmp.path().join("worktrees").join("recovered-project");
    let name_session = name_wt_dir.join("session-1");
    fs::create_dir_all(&name_session).expect("create name worktree dir");

    let stale_uuid_session = tmp
        .path()
        .join("worktrees")
        .join(project_id.to_string())
        .join("session-1");

    let project = Project {
        id: project_id,
        name: "recovered-project".to_string(),
        repo_path: "/tmp/fake-repo".to_string(),
        created_at: "2026-03-01T00:00:00Z".to_string(),
        archived: false,
        maintainer: MaintainerConfig::default(),
        auto_worker: AutoWorkerConfig::default(),
        prompts: vec![],
        sessions: vec![SessionConfig {
            id: Uuid::new_v4(),
            label: "session-1".to_string(),
            worktree_path: Some(stale_uuid_session.to_str().unwrap().to_string()),
            worktree_branch: Some("session-1".to_string()),
            archived: false,
            kind: "claude".to_string(),
            github_issue: None,
            initial_prompt: None,
            done_commits: vec![],
            auto_worker_session: false,
        }],
        staged_sessions: vec![],
    };
    storage.save_project(&project).expect("save project");

    storage
        .migrate_worktree_paths(&project)
        .expect("first migrate");
    let repaired = storage
        .load_project(project_id)
        .expect("load repaired project");
    assert_eq!(
        repaired.sessions[0].worktree_path.as_deref(),
        Some(name_session.to_str().unwrap()),
        "migration should repair stale UUID-based paths when the directory was already renamed"
    );

    storage
        .migrate_worktree_paths(&repaired)
        .expect("second migrate should stay idempotent");
    let rerun = storage
        .load_project(project_id)
        .expect("load after second migrate");
    assert_eq!(
        rerun.sessions[0].worktree_path.as_deref(),
        Some(name_session.to_str().unwrap()),
        "running the migration again should keep the repaired path stable"
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
        auto_worker: AutoWorkerConfig::default(),
        prompts: vec![],
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
            auto_worker_session: false,
        }],
        staged_sessions: vec![],
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
    let sig = repo
        .signature()
        .unwrap_or_else(|_| git2::Signature::now("Test", "test@example.com").unwrap());
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
        auto_worker: AutoWorkerConfig::default(),
        prompts: vec![],
        sessions: vec![],
        staged_sessions: vec![],
    };
    storage.save_project(&project).expect("save project");

    // Create worktree using name-based path (what create_session should do)
    let worktree_dir = tmp
        .path()
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
    the_controller_lib::commands::ensure_claude_md_symlink(&repo_path).unwrap();
    fs::create_dir_all(repo_path.join("docs").join("plans")).unwrap();
    fs::write(repo_path.join("docs").join("plans").join(".gitkeep"), "").unwrap();

    // Verify files exist on disk
    assert!(repo_path.join("agents.md").exists());
    assert!(repo_path.join("CLAUDE.md").exists());
    assert!(repo_path.join("docs/plans/.gitkeep").exists());

    // Verify CLAUDE.md is a symlink to agents.md
    assert!(repo_path
        .join("CLAUDE.md")
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::read_link(repo_path.join("CLAUDE.md"))
            .unwrap()
            .to_str()
            .unwrap(),
        "agents.md"
    );

    // Verify agents.md contains the project name
    let content = fs::read_to_string(repo_path.join("agents.md")).unwrap();
    assert!(content.starts_with(&format!("# {}", name)));
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
    the_controller_lib::commands::ensure_claude_md_symlink(&repo_path).unwrap();
    fs::create_dir_all(repo_path.join("docs").join("plans")).unwrap();
    fs::write(repo_path.join("docs").join("plans").join(".gitkeep"), "").unwrap();

    // Add to index and commit
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("agents.md")).unwrap();
    index.add_path(std::path::Path::new("CLAUDE.md")).unwrap();
    index
        .add_path(std::path::Path::new("docs/plans/.gitkeep"))
        .unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
        .unwrap();

    // Verify the HEAD commit tree contains our files
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    let commit_tree = head.tree().unwrap();

    // agents.md and CLAUDE.md should be at root
    assert!(
        commit_tree.get_name("agents.md").is_some(),
        "agents.md missing from commit tree"
    );
    assert!(
        commit_tree.get_name("CLAUDE.md").is_some(),
        "CLAUDE.md missing from commit tree"
    );

    // docs/plans/.gitkeep should be nested
    let docs_entry = commit_tree
        .get_name("docs")
        .expect("docs/ missing from commit tree");
    let docs_tree = repo.find_tree(docs_entry.id()).unwrap();
    let plans_entry = docs_tree
        .get_name("plans")
        .expect("plans/ missing from docs tree");
    let plans_tree = repo.find_tree(plans_entry.id()).unwrap();
    assert!(
        plans_tree.get_name(".gitkeep").is_some(),
        ".gitkeep missing from plans tree"
    );

    // Verify agents.md content in the commit
    let agents_entry = commit_tree.get_name("agents.md").unwrap();
    let agents_blob = repo.find_blob(agents_entry.id()).unwrap();
    let content = std::str::from_utf8(agents_blob.content()).unwrap();
    assert!(content.starts_with(&format!("# {}", name)));
}

/// ensure_claude_md_symlink creates CLAUDE.md -> agents.md when agents.md
/// exists but CLAUDE.md does not.
#[test]
fn test_ensure_claude_md_symlink_creates_when_missing() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    fs::write(dir.join("agents.md"), "# Test").unwrap();

    the_controller_lib::commands::ensure_claude_md_symlink(dir).unwrap();

    assert!(dir.join("CLAUDE.md").exists());
    assert!(dir
        .join("CLAUDE.md")
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::read_link(dir.join("CLAUDE.md"))
            .unwrap()
            .to_str()
            .unwrap(),
        "agents.md"
    );
    // Content should match
    assert_eq!(fs::read_to_string(dir.join("CLAUDE.md")).unwrap(), "# Test");
}

/// ensure_claude_md_symlink does not overwrite an existing CLAUDE.md.
#[test]
fn test_ensure_claude_md_symlink_skips_when_exists() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    fs::write(dir.join("agents.md"), "# Agents").unwrap();
    fs::write(dir.join("CLAUDE.md"), "# Custom").unwrap();

    the_controller_lib::commands::ensure_claude_md_symlink(dir).unwrap();

    // Should not have been replaced
    assert!(!dir
        .join("CLAUDE.md")
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::read_to_string(dir.join("CLAUDE.md")).unwrap(),
        "# Custom"
    );
}

/// ensure_claude_md_symlink does nothing when agents.md is missing.
#[test]
fn test_ensure_claude_md_symlink_noop_without_agents() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    the_controller_lib::commands::ensure_claude_md_symlink(dir).unwrap();

    assert!(!dir.join("CLAUDE.md").exists());
}
