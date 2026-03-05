# Worktrees Nested by Project Name — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Use human-readable project names instead of UUIDs for worktree directory nesting.

**Architecture:** Change `create_session()` to use `project.name` in the worktree path. Add a `migrate_worktree_paths()` function to `Storage` that renames existing UUID dirs and updates stored paths. Run migration in `restore_sessions()` before spawning PTYs.

**Tech Stack:** Rust (Tauri v2), git2, std::fs

---

### Task 1: Add `migrate_worktree_paths` to Storage

**Files:**
- Modify: `src-tauri/src/storage.rs:57-76` (after `list_projects`)
- Test: `src-tauri/tests/integration.rs`

**Step 1: Write the failing test**

Add to `src-tauri/tests/integration.rs`:

```rust
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
        sessions: vec![SessionConfig {
            id: Uuid::new_v4(),
            label: "session-1".to_string(),
            worktree_path: Some(session_wt.to_str().unwrap().to_string()),
            worktree_branch: Some("session-1".to_string()),
            archived: false,
            kind: "claude".to_string(),
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
fn test_migrate_worktree_paths_noop_when_already_migrated() {
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
        sessions: vec![SessionConfig {
            id: Uuid::new_v4(),
            label: "session-1".to_string(),
            worktree_path: Some(session_wt.to_str().unwrap().to_string()),
            worktree_branch: Some("session-1".to_string()),
            archived: false,
            kind: "claude".to_string(),
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
```

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test --test integration test_migrate_worktree_paths`
Expected: FAIL — `migrate_worktree_paths` doesn't exist

**Step 3: Implement `migrate_worktree_paths` in Storage**

Add to `src-tauri/src/storage.rs` after `list_projects`:

```rust
/// Migrate worktree directories from UUID-based to name-based paths.
///
/// Renames `worktrees/{project_uuid}/` to `worktrees/{project_name}/`
/// and updates all `worktree_path` entries in the project's sessions.
/// No-op if the UUID directory doesn't exist (already migrated or no worktrees).
pub fn migrate_worktree_paths(&self, project: &Project) -> std::io::Result<()> {
    let uuid_dir = self.base_dir.join("worktrees").join(project.id.to_string());
    if !uuid_dir.exists() {
        return Ok(());
    }

    let name_dir = self.base_dir.join("worktrees").join(&project.name);
    if name_dir.exists() {
        eprintln!(
            "Warning: cannot migrate worktrees for project '{}': target dir already exists",
            project.name
        );
        return Ok(());
    }

    // Rename the directory
    fs::rename(&uuid_dir, &name_dir)?;

    // Update stored worktree paths
    let uuid_prefix = uuid_dir.to_str().unwrap_or_default();
    let name_prefix = name_dir.to_str().unwrap_or_default();

    let mut updated = project.clone();
    for session in &mut updated.sessions {
        if let Some(ref wt_path) = session.worktree_path {
            if wt_path.starts_with(uuid_prefix) {
                session.worktree_path =
                    Some(wt_path.replacen(uuid_prefix, name_prefix, 1));
            }
        }
    }
    self.save_project(&updated)?;

    Ok(())
}
```

**Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --test integration test_migrate_worktree_paths`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/storage.rs src-tauri/tests/integration.rs
git commit -m "feat: add migrate_worktree_paths to rename UUID dirs to project names (#14)"
```

---

### Task 2: Run migration in `restore_sessions`

**Files:**
- Modify: `src-tauri/src/commands.rs:47-79` (`restore_sessions`)

**Step 1: Write the failing test**

Add to `src-tauri/tests/integration.rs`:

```rust
/// After migration, new sessions should use name-based paths.
/// This verifies the full flow: migrate then create.
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
```

**Step 2: Run test to verify it passes** (this tests the path shape, not the command itself)

Run: `cd src-tauri && cargo test --test integration test_create_session_uses_project_name`
Expected: PASS (proves the path construction works)

**Step 3: Update `create_session` and `restore_sessions` in commands.rs**

In `create_session()` at line 336, also extract `project.name`:

```rust
let (repo_path, label, base_dir, project_name) = {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let project = storage.load_project(project_uuid).map_err(|e| e.to_string())?;
    let label = next_session_label(&project.sessions);
    (project.repo_path.clone(), label, storage.base_dir(), project.name.clone())
};
```

At line 344, use `project_name` instead of `project_uuid.to_string()`:

```rust
let worktree_dir = base_dir
    .join("worktrees")
    .join(&project_name)
    .join(&label);
```

In `restore_sessions()`, add migration before spawning PTYs. After loading projects at line 51-54:

```rust
let projects = {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let projects = storage.list_projects().map_err(|e| e.to_string())?;
    // Migrate worktree paths from UUID-based to name-based directories
    for project in &projects {
        if let Err(e) = storage.migrate_worktree_paths(project) {
            eprintln!("Failed to migrate worktrees for project '{}': {}", project.name, e);
        }
    }
    // Reload after migration to get updated paths
    storage.list_projects().map_err(|e| e.to_string())?
};
```

**Step 4: Run all tests**

Run: `cd src-tauri && cargo test`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "feat: use project name in worktree paths and migrate on startup (#14)"
```

---

### Task 3: Manual verification

**Step 1: Check current worktree state**

```bash
ls -la ~/.the-controller/worktrees/
```

Expected: UUID-named directories exist

**Step 2: Run the app**

```bash
npm run tauri dev
```

**Step 3: Verify migration happened**

```bash
ls -la ~/.the-controller/worktrees/
```

Expected: Directory now named by project name (e.g., `the-controller/`) instead of UUID

**Step 4: Create a new session and verify path**

Create a new session in the app, then check:

```bash
ls -la ~/.the-controller/worktrees/the-controller/
```

Expected: New session directory (e.g., `session-3/`) appears under the project name directory

**Step 5: Commit (if any fixups needed)**
