# Agent Platform Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a Tauri desktop app that manages multiple Claude Code sessions across projects with embedded terminals and a sidebar GUI.

**Architecture:** Tauri v2 Rust backend manages PTYs (portable-pty) and project config (~/.the-controller/). Svelte 5 frontend renders a sidebar for project/session navigation and a single xterm.js terminal area that switches between sessions. IPC bridges the two via Tauri commands and events.

**Tech Stack:** Tauri v2, Rust, portable-pty, git2, Svelte 5, xterm.js, TypeScript

**Design doc:** `docs/plans/2026-02-28-agent-platform-design.md`

---

### Task 1: Scaffold Tauri + Svelte Project

**Files:**
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/build.rs`
- Create: `package.json`
- Create: `svelte.config.js`
- Create: `vite.config.ts`
- Create: `tsconfig.json`
- Create: `src/main.ts`
- Create: `src/App.svelte`
- Create: `src/app.css`
- Create: `index.html`

**Step 1: Install prerequisites and scaffold**

Run: `npm create tauri-app@latest . -- --template svelte-ts --manager npm`

If the directory is non-empty, run from a temp dir and move files in. The scaffolder sets up Tauri v2 + Svelte + Vite.

**Step 2: Add Rust dependencies**

Edit `src-tauri/Cargo.toml` to add these dependencies beyond what the scaffolder provides:

```toml
[dependencies]
portable-pty = "0.8"
git2 = "0.19"
uuid = { version = "1", features = ["v4", "serde"] }
tokio = { version = "1", features = ["full"] }
serde_json = "1"
dirs = "5"
```

**Step 3: Add frontend dependencies**

Run: `npm install @xterm/xterm @xterm/addon-fit`

**Step 4: Verify the app builds and opens**

Run: `npm run tauri dev`
Expected: A blank Tauri window opens with the default Svelte template.

**Step 5: Commit**

```bash
git add -A
git commit -m "scaffold: Tauri v2 + Svelte 5 project with dependencies"
```

---

### Task 2: Rust Data Model and Persistence

**Files:**
- Create: `src-tauri/src/models.rs`
- Create: `src-tauri/src/storage.rs`
- Modify: `src-tauri/src/main.rs`

**Step 1: Write failing tests for data model serialization**

Create `src-tauri/src/models.rs`:

```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub repo_path: String,
    pub created_at: String,
    pub archived: bool,
    pub sessions: Vec<SessionConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub id: Uuid,
    pub label: String,
    pub worktree_path: Option<String>,
    pub worktree_branch: Option<String>,
}

/// Session status is runtime-only, not persisted
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionStatus {
    Running,
    Idle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: Uuid,
    pub label: String,
    pub project_id: Uuid,
    pub worktree_path: Option<String>,
    pub worktree_branch: Option<String>,
    pub status: SessionStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_serialization_roundtrip() {
        let project = Project {
            id: Uuid::new_v4(),
            name: "test-project".to_string(),
            repo_path: "/tmp/test-repo".to_string(),
            created_at: "2026-02-28T00:00:00Z".to_string(),
            archived: false,
            sessions: vec![SessionConfig {
                id: Uuid::new_v4(),
                label: "main".to_string(),
                worktree_path: None,
                worktree_branch: None,
            }],
        };
        let json = serde_json::to_string(&project).unwrap();
        let deserialized: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "test-project");
        assert_eq!(deserialized.sessions.len(), 1);
        assert_eq!(deserialized.sessions[0].label, "main");
    }

    #[test]
    fn test_project_with_worktree_session() {
        let project = Project {
            id: Uuid::new_v4(),
            name: "feature-work".to_string(),
            repo_path: "/tmp/repo".to_string(),
            created_at: "2026-02-28T00:00:00Z".to_string(),
            archived: false,
            sessions: vec![SessionConfig {
                id: Uuid::new_v4(),
                label: "refine-auth".to_string(),
                worktree_path: Some("/tmp/repo/.worktrees/refine-auth".to_string()),
                worktree_branch: Some("refine-auth".to_string()),
            }],
        };
        let json = serde_json::to_string_pretty(&project).unwrap();
        let deserialized: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(
            deserialized.sessions[0].worktree_branch,
            Some("refine-auth".to_string())
        );
    }
}
```

**Step 2: Run tests to verify they pass**

Run: `cd src-tauri && cargo test`
Expected: 2 tests pass.

**Step 3: Write failing tests for storage layer**

Create `src-tauri/src/storage.rs`:

```rust
use crate::models::Project;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct Storage {
    base_dir: PathBuf,
}

impl Storage {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub fn default() -> Self {
        let base = dirs::home_dir()
            .expect("No home directory found")
            .join(".the-controller");
        Self::new(base)
    }

    pub fn ensure_dirs(&self) -> Result<(), String> {
        let projects_dir = self.base_dir.join("projects");
        fs::create_dir_all(&projects_dir).map_err(|e| e.to_string())
    }

    pub fn project_dir(&self, project_id: &Uuid) -> PathBuf {
        self.base_dir.join("projects").join(project_id.to_string())
    }

    pub fn save_project(&self, project: &Project) -> Result<(), String> {
        let dir = self.project_dir(&project.id);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join("project.json");
        let json = serde_json::to_string_pretty(project).map_err(|e| e.to_string())?;
        fs::write(path, json).map_err(|e| e.to_string())
    }

    pub fn load_project(&self, project_id: &Uuid) -> Result<Project, String> {
        let path = self.project_dir(project_id).join("project.json");
        let json = fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&json).map_err(|e| e.to_string())
    }

    pub fn list_projects(&self) -> Result<Vec<Project>, String> {
        let projects_dir = self.base_dir.join("projects");
        if !projects_dir.exists() {
            return Ok(vec![]);
        }
        let mut projects = vec![];
        let entries = fs::read_dir(&projects_dir).map_err(|e| e.to_string())?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let project_json = entry.path().join("project.json");
            if project_json.exists() {
                let json = fs::read_to_string(&project_json).map_err(|e| e.to_string())?;
                if let Ok(project) = serde_json::from_str::<Project>(&json) {
                    projects.push(project);
                }
            }
        }
        Ok(projects)
    }

    pub fn delete_project_dir(&self, project_id: &Uuid) -> Result<(), String> {
        let dir = self.project_dir(project_id);
        if dir.exists() {
            fs::remove_dir_all(dir).map_err(|e| e.to_string())
        } else {
            Ok(())
        }
    }

    /// Returns the effective agents.md content for a project.
    /// Checks repo root first, falls back to project config dir.
    pub fn get_agents_md(&self, project: &Project) -> Result<String, String> {
        let repo_agents = Path::new(&project.repo_path).join("agents.md");
        if repo_agents.exists() {
            return fs::read_to_string(repo_agents).map_err(|e| e.to_string());
        }
        let local_agents = self.project_dir(&project.id).join("agents.md");
        if local_agents.exists() {
            return fs::read_to_string(local_agents).map_err(|e| e.to_string());
        }
        Ok(String::new())
    }

    pub fn save_agents_md(&self, project_id: &Uuid, content: &str) -> Result<(), String> {
        let path = self.project_dir(project_id).join("agents.md");
        fs::write(path, content).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Project;
    use tempfile::TempDir;

    fn test_storage() -> (Storage, TempDir) {
        let tmp = TempDir::new().unwrap();
        let storage = Storage::new(tmp.path().to_path_buf());
        storage.ensure_dirs().unwrap();
        (storage, tmp)
    }

    fn test_project() -> Project {
        Project {
            id: Uuid::new_v4(),
            name: "test".to_string(),
            repo_path: "/tmp/fake-repo".to_string(),
            created_at: "2026-02-28T00:00:00Z".to_string(),
            archived: false,
            sessions: vec![],
        }
    }

    #[test]
    fn test_save_and_load_project() {
        let (storage, _tmp) = test_storage();
        let project = test_project();
        storage.save_project(&project).unwrap();
        let loaded = storage.load_project(&project.id).unwrap();
        assert_eq!(loaded.name, "test");
        assert_eq!(loaded.id, project.id);
    }

    #[test]
    fn test_list_projects() {
        let (storage, _tmp) = test_storage();
        let p1 = test_project();
        let mut p2 = test_project();
        p2.name = "second".to_string();
        storage.save_project(&p1).unwrap();
        storage.save_project(&p2).unwrap();
        let projects = storage.list_projects().unwrap();
        assert_eq!(projects.len(), 2);
    }

    #[test]
    fn test_list_empty() {
        let (storage, _tmp) = test_storage();
        let projects = storage.list_projects().unwrap();
        assert_eq!(projects.len(), 0);
    }

    #[test]
    fn test_agents_md_fallback() {
        let (storage, _tmp) = test_storage();
        let project = test_project();
        storage.save_project(&project).unwrap();

        // No agents.md anywhere → empty string
        let content = storage.get_agents_md(&project).unwrap();
        assert_eq!(content, "");

        // Save local agents.md → returns it
        storage.save_agents_md(&project.id, "# Instructions").unwrap();
        let content = storage.get_agents_md(&project).unwrap();
        assert_eq!(content, "# Instructions");
    }

    #[test]
    fn test_agents_md_repo_takes_priority() {
        let (storage, _tmp) = test_storage();
        let repo_dir = TempDir::new().unwrap();
        let mut project = test_project();
        project.repo_path = repo_dir.path().to_string_lossy().to_string();
        storage.save_project(&project).unwrap();

        // Write agents.md in both places
        storage.save_agents_md(&project.id, "local fallback").unwrap();
        fs::write(repo_dir.path().join("agents.md"), "repo version").unwrap();

        // Repo version wins
        let content = storage.get_agents_md(&project).unwrap();
        assert_eq!(content, "repo version");
    }

    #[test]
    fn test_delete_project() {
        let (storage, _tmp) = test_storage();
        let project = test_project();
        storage.save_project(&project).unwrap();
        assert!(storage.load_project(&project.id).is_ok());
        storage.delete_project_dir(&project.id).unwrap();
        assert!(storage.load_project(&project.id).is_err());
    }
}
```

**Step 4: Add tempfile dev dependency**

Add to `src-tauri/Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"
```

**Step 5: Wire modules into main.rs**

Add to top of `src-tauri/src/main.rs`:

```rust
mod models;
mod storage;
```

**Step 6: Run tests**

Run: `cd src-tauri && cargo test`
Expected: All 7 tests pass (2 model + 5 storage).

**Step 7: Commit**

```bash
git add src-tauri/src/models.rs src-tauri/src/storage.rs src-tauri/src/main.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat: add data model and storage layer with tests"
```

---

### Task 3: Tauri Commands — Project Management

**Files:**
- Create: `src-tauri/src/commands.rs`
- Create: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/main.rs`

**Step 1: Create app state**

Create `src-tauri/src/state.rs`:

```rust
use crate::storage::Storage;
use std::sync::Mutex;

pub struct AppState {
    pub storage: Mutex<Storage>,
}

impl AppState {
    pub fn new() -> Self {
        let storage = Storage::default();
        storage.ensure_dirs().unwrap();
        Self {
            storage: Mutex::new(storage),
        }
    }
}
```

**Step 2: Create Tauri commands for project CRUD**

Create `src-tauri/src/commands.rs`:

```rust
use crate::models::{Project, SessionConfig};
use crate::state::AppState;
use std::fs;
use std::path::Path;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub fn create_project(
    state: State<AppState>,
    name: String,
    repo_path: String,
) -> Result<Project, String> {
    // Validate repo_path exists and is a directory
    if !Path::new(&repo_path).is_dir() {
        return Err(format!("Directory does not exist: {}", repo_path));
    }

    let project = Project {
        id: Uuid::new_v4(),
        name,
        repo_path: repo_path.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        archived: false,
        sessions: vec![],
    };

    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    storage.save_project(&project)?;

    // Create default agents.md if repo doesn't have one
    let repo_agents = Path::new(&repo_path).join("agents.md");
    if !repo_agents.exists() {
        let default_agents = "# Agent Instructions\n\n1. **Definition**: What's the task? Why are we doing it?\n2. **Constraints**: What are the design constraints?\n3. **Validation**: How do I validate it works?\n";
        storage.save_agents_md(&project.id, default_agents)?;
    }

    Ok(project)
}

#[tauri::command]
pub fn load_project(
    state: State<AppState>,
    name: String,
    repo_path: String,
) -> Result<Project, String> {
    if !Path::new(&repo_path).is_dir() {
        return Err(format!("Directory does not exist: {}", repo_path));
    }

    // Check if it's a git repo
    let git_dir = Path::new(&repo_path).join(".git");
    if !git_dir.exists() {
        return Err(format!("Not a git repository: {}", repo_path));
    }

    let project = Project {
        id: Uuid::new_v4(),
        name,
        repo_path: repo_path.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        archived: false,
        sessions: vec![],
    };

    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    storage.save_project(&project)?;

    // Only create default agents.md if repo doesn't have one
    let repo_agents = Path::new(&repo_path).join("agents.md");
    if !repo_agents.exists() {
        let default_agents = "# Agent Instructions\n\n1. **Definition**: What's the task? Why are we doing it?\n2. **Constraints**: What are the design constraints?\n3. **Validation**: How do I validate it works?\n";
        storage.save_agents_md(&project.id, default_agents)?;
    }

    Ok(project)
}

#[tauri::command]
pub fn list_projects(state: State<AppState>) -> Result<Vec<Project>, String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let projects = storage.list_projects()?;
    // Filter out archived by default
    Ok(projects.into_iter().filter(|p| !p.archived).collect())
}

#[tauri::command]
pub fn archive_project(state: State<AppState>, project_id: String) -> Result<(), String> {
    let id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage.load_project(&id)?;
    project.archived = true;
    project.sessions.clear();
    storage.save_project(&project)
}

#[tauri::command]
pub fn get_agents_md(state: State<AppState>, project_id: String) -> Result<String, String> {
    let id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let project = storage.load_project(&id)?;
    storage.get_agents_md(&project)
}

#[tauri::command]
pub fn update_agents_md(
    state: State<AppState>,
    project_id: String,
    content: String,
) -> Result<(), String> {
    let id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    storage.save_agents_md(&id, &content)
}
```

**Step 3: Add chrono dependency**

Add to `src-tauri/Cargo.toml` dependencies:

```toml
chrono = { version = "0.4", features = ["serde"] }
```

**Step 4: Register commands and state in main.rs**

Update `src-tauri/src/main.rs`:

```rust
mod commands;
mod models;
mod state;
mod storage;

fn main() {
    tauri::Builder::default()
        .manage(state::AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::create_project,
            commands::load_project,
            commands::list_projects,
            commands::archive_project,
            commands::get_agents_md,
            commands::update_agents_md,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 5: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: Compiles with no errors.

**Step 6: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/state.rs src-tauri/src/main.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat: add Tauri commands for project management"
```

---

### Task 4: PTY Manager

**Files:**
- Create: `src-tauri/src/pty_manager.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/main.rs`

**Step 1: Create PTY manager struct**

Create `src-tauri/src/pty_manager.rs`:

```rust
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

pub struct PtySession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    pub alive: Arc<Mutex<bool>>,
}

pub struct PtyManager {
    sessions: HashMap<Uuid, PtySession>,
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn spawn_session(
        &mut self,
        session_id: Uuid,
        working_dir: &str,
        app_handle: &AppHandle,
    ) -> Result<(), String> {
        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())?;

        let mut cmd = CommandBuilder::new("claude");
        cmd.cwd(working_dir);

        let _child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
        drop(pair.slave); // Release slave side

        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
        let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;

        let alive = Arc::new(Mutex::new(true));
        let alive_clone = alive.clone();
        let event_id = session_id;
        let handle = app_handle.clone();

        // Spawn reader thread that emits events
        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        *alive_clone.lock().unwrap() = false;
                        let _ = handle.emit(
                            &format!("session-status-changed:{}", event_id),
                            "idle",
                        );
                        break;
                    }
                    Ok(n) => {
                        let data = &buf[..n];
                        // Send as base64 to avoid UTF-8 issues with raw terminal output
                        use base64::Engine;
                        let encoded =
                            base64::engine::general_purpose::STANDARD.encode(data);
                        let _ = handle.emit(
                            &format!("pty-output:{}", event_id),
                            encoded,
                        );
                    }
                    Err(_) => {
                        *alive_clone.lock().unwrap() = false;
                        let _ = handle.emit(
                            &format!("session-status-changed:{}", event_id),
                            "idle",
                        );
                        break;
                    }
                }
            }
        });

        self.sessions.insert(
            session_id,
            PtySession {
                master: pair.master,
                writer,
                alive,
            },
        );

        Ok(())
    }

    pub fn write_to_session(&mut self, session_id: &Uuid, data: &[u8]) -> Result<(), String> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or("Session not found")?;
        session.writer.write_all(data).map_err(|e| e.to_string())
    }

    pub fn resize_session(
        &mut self,
        session_id: &Uuid,
        rows: u16,
        cols: u16,
    ) -> Result<(), String> {
        let session = self.sessions.get(session_id).ok_or("Session not found")?;
        session
            .master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())
    }

    pub fn is_alive(&self, session_id: &Uuid) -> bool {
        self.sessions
            .get(session_id)
            .map(|s| *s.alive.lock().unwrap())
            .unwrap_or(false)
    }

    pub fn close_session(&mut self, session_id: &Uuid) -> Result<(), String> {
        self.sessions.remove(session_id);
        Ok(())
    }
}
```

**Step 2: Add base64 dependency**

Add to `src-tauri/Cargo.toml`:

```toml
base64 = "0.22"
```

**Step 3: Update AppState to include PtyManager**

Update `src-tauri/src/state.rs`:

```rust
use crate::pty_manager::PtyManager;
use crate::storage::Storage;
use std::sync::Mutex;

pub struct AppState {
    pub storage: Mutex<Storage>,
    pub pty_manager: Mutex<PtyManager>,
}

impl AppState {
    pub fn new() -> Self {
        let storage = Storage::default();
        storage.ensure_dirs().unwrap();
        Self {
            storage: Mutex::new(storage),
            pty_manager: Mutex::new(PtyManager::new()),
        }
    }
}
```

**Step 4: Add session commands to commands.rs**

Append to `src-tauri/src/commands.rs`:

```rust
#[tauri::command]
pub fn create_session(
    state: State<AppState>,
    app_handle: tauri::AppHandle,
    project_id: String,
    label: String,
) -> Result<String, String> {
    let proj_id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage.load_project(&proj_id)?;

    let session_id = Uuid::new_v4();
    let session_config = SessionConfig {
        id: session_id,
        label,
        worktree_path: None,
        worktree_branch: None,
    };
    project.sessions.push(session_config);
    storage.save_project(&project)?;

    let working_dir = project.repo_path.clone();
    drop(storage);

    let mut pty_mgr = state.pty_manager.lock().map_err(|e| e.to_string())?;
    pty_mgr.spawn_session(session_id, &working_dir, &app_handle)?;

    Ok(session_id.to_string())
}

#[tauri::command]
pub fn write_to_pty(
    state: State<AppState>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    let id = Uuid::parse_str(&session_id).map_err(|e| e.to_string())?;
    let mut pty_mgr = state.pty_manager.lock().map_err(|e| e.to_string())?;
    pty_mgr.write_to_session(&id, data.as_bytes())
}

#[tauri::command]
pub fn resize_pty(
    state: State<AppState>,
    session_id: String,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    let id = Uuid::parse_str(&session_id).map_err(|e| e.to_string())?;
    let mut pty_mgr = state.pty_manager.lock().map_err(|e| e.to_string())?;
    pty_mgr.resize_session(&id, rows, cols)
}

#[tauri::command]
pub fn close_session(
    state: State<AppState>,
    project_id: String,
    session_id: String,
) -> Result<(), String> {
    let sess_id = Uuid::parse_str(&session_id).map_err(|e| e.to_string())?;
    let proj_id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let mut pty_mgr = state.pty_manager.lock().map_err(|e| e.to_string())?;
    pty_mgr.close_session(&sess_id)?;
    drop(pty_mgr);

    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage.load_project(&proj_id)?;
    project.sessions.retain(|s| s.id != sess_id);
    storage.save_project(&project)
}
```

**Step 5: Register new commands in main.rs**

Update invoke_handler in `src-tauri/src/main.rs` to include:

```rust
commands::create_session,
commands::write_to_pty,
commands::resize_pty,
commands::close_session,
```

And add `mod pty_manager;` to the module declarations.

**Step 6: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: Compiles with no errors.

**Step 7: Commit**

```bash
git add src-tauri/
git commit -m "feat: add PTY manager for spawning and managing Claude Code sessions"
```

---

### Task 5: Frontend — Sidebar Component

**Files:**
- Create: `src/lib/Sidebar.svelte`
- Create: `src/lib/stores.ts`
- Modify: `src/App.svelte`
- Modify: `src/app.css`

**Step 1: Create reactive stores**

Create `src/lib/stores.ts`:

```typescript
import { writable } from "svelte/store";

export interface SessionConfig {
  id: string;
  label: string;
  worktree_path: string | null;
  worktree_branch: string | null;
}

export interface Project {
  id: string;
  name: string;
  repo_path: string;
  created_at: string;
  archived: boolean;
  sessions: SessionConfig[];
}

export interface SessionRuntime {
  id: string;
  projectId: string;
  status: "running" | "idle";
}

export const projects = writable<Project[]>([]);
export const activeSessionId = writable<string | null>(null);
export const sessionStatuses = writable<Map<string, "running" | "idle">>(
  new Map()
);
```

**Step 2: Create Sidebar component**

Create `src/lib/Sidebar.svelte`:

```svelte
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import {
    projects,
    activeSessionId,
    sessionStatuses,
    type Project,
  } from "./stores";

  let showNewModal = $state(false);
  let newProjectName = $state("");
  let newProjectPath = $state("");
  let expandedProjects = $state<Set<string>>(new Set());

  async function loadProjects() {
    const result = await invoke<Project[]>("list_projects");
    projects.set(result);
  }

  async function createProject() {
    await invoke("create_project", {
      name: newProjectName,
      repoPath: newProjectPath,
    });
    showNewModal = false;
    newProjectName = "";
    newProjectPath = "";
    await loadProjects();
  }

  async function createSession(projectId: string) {
    const sessionId = await invoke<string>("create_session", {
      projectId,
      label: `session-${Date.now()}`,
    });
    activeSessionId.set(sessionId);
    sessionStatuses.update((m) => {
      m.set(sessionId, "running");
      return m;
    });
    await loadProjects();
  }

  function toggleProject(id: string) {
    expandedProjects = expandedProjects.has(id)
      ? (expandedProjects.delete(id), new Set(expandedProjects))
      : new Set(expandedProjects.add(id));
  }

  function selectSession(sessionId: string) {
    activeSessionId.set(sessionId);
  }

  // Load on mount
  loadProjects();
</script>

<div class="sidebar">
  <div class="sidebar-header">
    <h2>Projects</h2>
    <button class="btn-new" onclick={() => (showNewModal = !showNewModal)}>
      + New
    </button>
  </div>

  {#if showNewModal}
    <div class="new-project-form">
      <input bind:value={newProjectName} placeholder="Project name" />
      <input bind:value={newProjectPath} placeholder="Repo path" />
      <div class="form-actions">
        <button onclick={createProject}>Create</button>
        <button onclick={() => (showNewModal = false)}>Cancel</button>
      </div>
    </div>
  {/if}

  <div class="project-list">
    {#each $projects as project}
      <div class="project-item">
        <div
          class="project-header"
          onclick={() => toggleProject(project.id)}
          role="button"
          tabindex="0"
        >
          <span class="expand-icon">
            {expandedProjects.has(project.id) ? "▼" : "▶"}
          </span>
          <span class="project-name">{project.name}</span>
          <span class="session-count">{project.sessions.length}</span>
          <button
            class="btn-add-session"
            onclick|stopPropagation={() => createSession(project.id)}
          >
            +
          </button>
        </div>

        {#if expandedProjects.has(project.id)}
          <div class="session-list">
            {#each project.sessions as session}
              {@const status = $sessionStatuses.get(session.id) ?? "idle"}
              <div
                class="session-item"
                class:active={$activeSessionId === session.id}
                onclick={() => selectSession(session.id)}
                role="button"
                tabindex="0"
              >
                <span class="status-dot" class:running={status === "running"}>
                  {status === "running" ? "●" : "○"}
                </span>
                <span class="session-label">{session.label}</span>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .sidebar {
    width: 250px;
    height: 100vh;
    background: #1e1e2e;
    color: #cdd6f4;
    display: flex;
    flex-direction: column;
    border-right: 1px solid #313244;
    overflow-y: auto;
  }
  .sidebar-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 16px;
    border-bottom: 1px solid #313244;
  }
  .sidebar-header h2 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
  }
  .btn-new {
    background: #45475a;
    color: #cdd6f4;
    border: none;
    padding: 4px 10px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 12px;
  }
  .new-project-form {
    padding: 12px 16px;
    border-bottom: 1px solid #313244;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .new-project-form input {
    background: #313244;
    color: #cdd6f4;
    border: 1px solid #45475a;
    padding: 6px 8px;
    border-radius: 4px;
    font-size: 12px;
  }
  .form-actions {
    display: flex;
    gap: 8px;
  }
  .form-actions button {
    background: #45475a;
    color: #cdd6f4;
    border: none;
    padding: 4px 10px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 12px;
  }
  .project-header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 16px;
    cursor: pointer;
    user-select: none;
  }
  .project-header:hover {
    background: #313244;
  }
  .expand-icon {
    font-size: 10px;
    width: 12px;
  }
  .project-name {
    flex: 1;
    font-size: 13px;
  }
  .session-count {
    font-size: 11px;
    color: #6c7086;
  }
  .btn-add-session {
    background: none;
    border: none;
    color: #6c7086;
    cursor: pointer;
    font-size: 14px;
    padding: 0 4px;
  }
  .session-list {
    padding-left: 24px;
  }
  .session-item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 16px;
    cursor: pointer;
    font-size: 12px;
    border-radius: 4px;
  }
  .session-item:hover {
    background: #313244;
  }
  .session-item.active {
    background: #45475a;
  }
  .status-dot {
    color: #6c7086;
    font-size: 10px;
  }
  .status-dot.running {
    color: #a6e3a1;
  }
</style>
```

**Step 3: Update App.svelte to use sidebar layout**

Replace `src/App.svelte`:

```svelte
<script lang="ts">
  import Sidebar from "./lib/Sidebar.svelte";
  import { activeSessionId } from "./lib/stores";
</script>

<div class="app-layout">
  <Sidebar />
  <main class="terminal-area">
    {#if $activeSessionId}
      <div class="terminal-placeholder">
        Terminal for session: {$activeSessionId}
      </div>
    {:else}
      <div class="empty-state">
        Select or create a session to begin.
      </div>
    {/if}
  </main>
</div>

<style>
  .app-layout {
    display: flex;
    height: 100vh;
    background: #11111b;
  }
  .terminal-area {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #6c7086;
    font-size: 14px;
  }
</style>
```

**Step 4: Update app.css for global resets**

Replace `src/app.css`:

```css
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}
body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  background: #11111b;
  overflow: hidden;
}
```

**Step 5: Verify the app renders**

Run: `npm run tauri dev`
Expected: Window shows sidebar on the left with "Projects" header and "+ New" button. Main area says "Select or create a session to begin."

**Step 6: Commit**

```bash
git add src/
git commit -m "feat: add sidebar component with project/session navigation"
```

---

### Task 6: Frontend — Terminal Area with xterm.js

**Files:**
- Create: `src/lib/Terminal.svelte`
- Modify: `src/App.svelte`

**Step 1: Create Terminal component**

Create `src/lib/Terminal.svelte`:

```svelte
<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import "@xterm/xterm/css/xterm.css";

  interface Props {
    sessionId: string;
  }

  let { sessionId }: Props = $props();

  let terminalEl: HTMLDivElement;
  let term: Terminal;
  let fitAddon: FitAddon;
  let unlistenOutput: UnlistenFn;
  let unlistenStatus: UnlistenFn;
  let resizeObserver: ResizeObserver;

  onMount(async () => {
    term = new Terminal({
      cursorBlink: true,
      fontSize: 13,
      fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
      theme: {
        background: "#11111b",
        foreground: "#cdd6f4",
        cursor: "#f5e0dc",
        selectionBackground: "#45475a",
      },
    });

    fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(terminalEl);
    fitAddon.fit();

    // Send user input to PTY
    term.onData((data) => {
      invoke("write_to_pty", { sessionId, data });
    });

    // Listen for PTY output
    unlistenOutput = await listen<string>(
      `pty-output:${sessionId}`,
      (event) => {
        const bytes = Uint8Array.from(atob(event.payload), (c) =>
          c.charCodeAt(0)
        );
        term.write(bytes);
      }
    );

    // Listen for status changes
    unlistenStatus = await listen<string>(
      `session-status-changed:${sessionId}`,
      (_event) => {
        term.write("\r\n\x1b[90m[Session ended]\x1b[0m\r\n");
      }
    );

    // Handle resize
    resizeObserver = new ResizeObserver(() => {
      fitAddon.fit();
      invoke("resize_pty", {
        sessionId,
        rows: term.rows,
        cols: term.cols,
      });
    });
    resizeObserver.observe(terminalEl);
  });

  onDestroy(() => {
    unlistenOutput?.();
    unlistenStatus?.();
    resizeObserver?.disconnect();
    term?.dispose();
  });
</script>

<div class="terminal-container" bind:this={terminalEl}></div>

<style>
  .terminal-container {
    width: 100%;
    height: 100%;
    padding: 4px;
  }
  .terminal-container :global(.xterm) {
    height: 100%;
  }
</style>
```

**Step 2: Wire Terminal into App.svelte**

Update `src/App.svelte`:

```svelte
<script lang="ts">
  import Sidebar from "./lib/Sidebar.svelte";
  import Terminal from "./lib/Terminal.svelte";
  import { activeSessionId } from "./lib/stores";
</script>

<div class="app-layout">
  <Sidebar />
  <main class="terminal-area">
    {#if $activeSessionId}
      {#key $activeSessionId}
        <Terminal sessionId={$activeSessionId} />
      {/key}
    {:else}
      <div class="empty-state">
        Select or create a session to begin.
      </div>
    {/if}
  </main>
</div>

<style>
  .app-layout {
    display: flex;
    height: 100vh;
    background: #11111b;
  }
  .terminal-area {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #6c7086;
    font-size: 14px;
  }
</style>
```

Note: The `{#key}` block ensures the Terminal component remounts when switching sessions, giving each session a fresh xterm.js instance. PTY output continues in the background regardless.

**Step 3: Verify terminal renders and connects**

Run: `npm run tauri dev`
Test: Create a project, create a session. The terminal should appear and show Claude Code launching.

**Step 4: Commit**

```bash
git add src/
git commit -m "feat: add xterm.js terminal component with PTY I/O"
```

---

### Task 7: Terminal Session Persistence Across Switches

The `{#key}` approach in Task 6 destroys and recreates the Terminal on switch, which loses scrollback. We need to keep xterm instances alive and swap visibility.

**Files:**
- Create: `src/lib/TerminalManager.svelte`
- Modify: `src/App.svelte`

**Step 1: Create TerminalManager that holds all active terminals**

Create `src/lib/TerminalManager.svelte`:

```svelte
<script lang="ts">
  import Terminal from "./Terminal.svelte";
  import { projects, activeSessionId } from "./stores";

  // Derive all session IDs from all projects
  let allSessionIds = $derived(
    $projects.flatMap((p) => p.sessions.map((s) => s.id))
  );
</script>

<div class="terminal-manager">
  {#each allSessionIds as sessionId (sessionId)}
    <div
      class="terminal-wrapper"
      class:visible={$activeSessionId === sessionId}
    >
      <Terminal {sessionId} />
    </div>
  {/each}

  {#if !$activeSessionId}
    <div class="empty-state">Select or create a session to begin.</div>
  {/if}
</div>

<style>
  .terminal-manager {
    width: 100%;
    height: 100%;
    position: relative;
  }
  .terminal-wrapper {
    position: absolute;
    inset: 0;
    display: none;
  }
  .terminal-wrapper.visible {
    display: block;
  }
  .empty-state {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #6c7086;
    font-size: 14px;
  }
</style>
```

**Step 2: Update App.svelte to use TerminalManager**

```svelte
<script lang="ts">
  import Sidebar from "./lib/Sidebar.svelte";
  import TerminalManager from "./lib/TerminalManager.svelte";
</script>

<div class="app-layout">
  <Sidebar />
  <main class="terminal-area">
    <TerminalManager />
  </main>
</div>

<style>
  .app-layout {
    display: flex;
    height: 100vh;
    background: #11111b;
  }
  .terminal-area {
    flex: 1;
  }
</style>
```

**Step 3: Update Terminal.svelte to refit on visibility change**

Add to the `onMount` in `src/lib/Terminal.svelte`, after the ResizeObserver setup:

```typescript
// Refit when becoming visible (display: none → block doesn't trigger ResizeObserver)
const mutationObserver = new MutationObserver(() => {
  if (terminalEl.offsetParent !== null) {
    fitAddon.fit();
  }
});
mutationObserver.observe(terminalEl.parentElement!, {
  attributes: true,
  attributeFilter: ["class"],
});
```

And clean up in `onDestroy`:

```typescript
mutationObserver?.disconnect();
```

**Step 4: Verify switching preserves scrollback**

Run: `npm run tauri dev`
Test: Create two sessions. Type in one, switch to the other, switch back. Output should be preserved.

**Step 5: Commit**

```bash
git add src/
git commit -m "feat: persist terminal instances across session switches"
```

---

### Task 8: Worktree Management

**Files:**
- Create: `src-tauri/src/worktree.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src/lib/Sidebar.svelte`

**Step 1: Write worktree manager with tests**

Create `src-tauri/src/worktree.rs`:

```rust
use git2::Repository;
use std::path::{Path, PathBuf};

pub struct WorktreeManager;

impl WorktreeManager {
    pub fn create_worktree(
        repo_path: &str,
        branch_name: &str,
    ) -> Result<PathBuf, String> {
        let repo = Repository::open(repo_path).map_err(|e| e.to_string())?;

        let worktree_dir = Path::new(repo_path)
            .join(".worktrees")
            .join(branch_name);

        if worktree_dir.exists() {
            return Err(format!(
                "Worktree directory already exists: {}",
                worktree_dir.display()
            ));
        }

        std::fs::create_dir_all(worktree_dir.parent().unwrap())
            .map_err(|e| e.to_string())?;

        // Create branch from HEAD
        let head = repo.head().map_err(|e| e.to_string())?;
        let commit = head.peel_to_commit().map_err(|e| e.to_string())?;
        repo.branch(branch_name, &commit, false)
            .map_err(|e| e.to_string())?;

        // Create worktree
        repo.worktree(
            branch_name,
            &worktree_dir,
            Some(
                git2::WorktreeAddOptions::new()
                    .reference(Some(&format!("refs/heads/{}", branch_name))),
            ),
        )
        .map_err(|e| e.to_string())?;

        Ok(worktree_dir)
    }

    pub fn remove_worktree(
        repo_path: &str,
        branch_name: &str,
    ) -> Result<(), String> {
        let worktree_dir = Path::new(repo_path)
            .join(".worktrees")
            .join(branch_name);

        if worktree_dir.exists() {
            std::fs::remove_dir_all(&worktree_dir).map_err(|e| e.to_string())?;
        }

        // Prune the worktree reference
        let repo = Repository::open(repo_path).map_err(|e| e.to_string())?;
        if let Ok(wt) = repo.find_worktree(branch_name) {
            wt.prune(Some(
                git2::WorktreePruneOptions::new()
                    .working_tree(true)
                    .valid(true),
            ))
            .map_err(|e| e.to_string())?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Repository;
    use tempfile::TempDir;

    fn setup_test_repo() -> (TempDir, String) {
        let tmp = TempDir::new().unwrap();
        let repo = Repository::init(tmp.path()).unwrap();

        // Create initial commit so HEAD exists
        let sig = repo.signature().unwrap_or_else(|_| {
            git2::Signature::now("Test", "test@test.com").unwrap()
        });
        let tree_id = repo.index().unwrap().write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
            .unwrap();

        let path = tmp.path().to_string_lossy().to_string();
        (tmp, path)
    }

    #[test]
    fn test_create_and_remove_worktree() {
        let (_tmp, repo_path) = setup_test_repo();

        let wt_path =
            WorktreeManager::create_worktree(&repo_path, "test-branch").unwrap();
        assert!(wt_path.exists());
        assert!(wt_path.join(".git").exists());

        WorktreeManager::remove_worktree(&repo_path, "test-branch").unwrap();
        assert!(!wt_path.exists());
    }

    #[test]
    fn test_duplicate_worktree_fails() {
        let (_tmp, repo_path) = setup_test_repo();

        WorktreeManager::create_worktree(&repo_path, "dupe").unwrap();
        let result = WorktreeManager::create_worktree(&repo_path, "dupe");
        assert!(result.is_err());
    }
}
```

**Step 2: Run worktree tests**

Run: `cd src-tauri && cargo test worktree`
Expected: 2 tests pass.

**Step 3: Add create_refinement command**

Add to `src-tauri/src/commands.rs`:

```rust
use crate::worktree::WorktreeManager;

#[tauri::command]
pub fn create_refinement(
    state: State<AppState>,
    app_handle: tauri::AppHandle,
    project_id: String,
    branch_name: String,
) -> Result<String, String> {
    let proj_id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage.load_project(&proj_id)?;

    let worktree_path =
        WorktreeManager::create_worktree(&project.repo_path, &branch_name)?;
    let worktree_str = worktree_path.to_string_lossy().to_string();

    let session_id = Uuid::new_v4();
    let session_config = SessionConfig {
        id: session_id,
        label: branch_name.clone(),
        worktree_path: Some(worktree_str.clone()),
        worktree_branch: Some(branch_name),
    };
    project.sessions.push(session_config);
    storage.save_project(&project)?;
    drop(storage);

    let mut pty_mgr = state.pty_manager.lock().map_err(|e| e.to_string())?;
    pty_mgr.spawn_session(session_id, &worktree_str, &app_handle)?;

    Ok(session_id.to_string())
}
```

**Step 4: Register command and module in main.rs**

Add `mod worktree;` and `commands::create_refinement` to the invoke handler.

**Step 5: Update Sidebar with refinement option**

Update the `createSession` area in `src/lib/Sidebar.svelte` to show a context menu or two buttons: "Session" and "Refinement". When "Refinement" is chosen, prompt for a branch name and call `create_refinement` instead of `create_session`.

Add to the `<script>` section:

```typescript
async function createRefinement(projectId: string) {
  const branchName = prompt("Branch name for refinement:");
  if (!branchName) return;
  const sessionId = await invoke<string>("create_refinement", {
    projectId,
    branchName,
  });
  activeSessionId.set(sessionId);
  sessionStatuses.update((m) => {
    m.set(sessionId, "running");
    return m;
  });
  await loadProjects();
}
```

Update the `btn-add-session` to be a dropdown or two adjacent buttons.

**Step 6: Run all tests**

Run: `cd src-tauri && cargo test`
Expected: All tests pass (model + storage + worktree).

**Step 7: Verify end-to-end**

Run: `npm run tauri dev`
Test: Create a project pointing at a real git repo, create a refinement with a branch name, verify worktree is created and Claude Code launches in it.

**Step 8: Commit**

```bash
git add src-tauri/ src/
git commit -m "feat: add worktree management for refinement sessions"
```

---

### Task 9: Session Status Tracking

**Files:**
- Modify: `src/lib/stores.ts`
- Modify: `src/lib/Sidebar.svelte`

**Step 1: Listen for status events globally**

Update `src/lib/Sidebar.svelte` to listen for `session-status-changed:*` events on mount:

```typescript
import { listen } from "@tauri-apps/api/event";

// In loadProjects or onMount:
// For each session in each project, listen for status changes
$effect(() => {
  const unlisteners: (() => void)[] = [];
  for (const project of $projects) {
    for (const session of project.sessions) {
      listen<string>(`session-status-changed:${session.id}`, () => {
        sessionStatuses.update((m) => {
          m.set(session.id, "idle");
          return m;
        });
      }).then((unlisten) => unlisteners.push(unlisten));
    }
  }
  return () => unlisteners.forEach((fn) => fn());
});
```

**Step 2: Initialize new sessions as running**

Already handled in `createSession` and `createRefinement` — they set status to "running" on creation.

**Step 3: Verify status indicators update**

Run: `npm run tauri dev`
Test: Create a session, see `●` (green). Exit Claude Code in the terminal, see it change to `○` (gray).

**Step 4: Commit**

```bash
git add src/
git commit -m "feat: track session status via PTY lifecycle events"
```

---

### Task 10: Archive and Close Flows

**Files:**
- Modify: `src/lib/Sidebar.svelte`
- Modify: `src-tauri/src/commands.rs`

**Step 1: Add close session UI**

In `src/lib/Sidebar.svelte`, add an `x` button on each session item:

```svelte
<button
  class="btn-close"
  onclick|stopPropagation={() => closeSession(project.id, session.id, session.worktree_branch)}
>
  ×
</button>
```

```typescript
async function closeSession(
  projectId: string,
  sessionId: string,
  worktreeBranch: string | null
) {
  await invoke("close_session", { projectId, sessionId });
  if (worktreeBranch) {
    // Worktree cleanup is handled by the backend close_session
  }
  sessionStatuses.update((m) => {
    m.delete(sessionId);
    return m;
  });
  if ($activeSessionId === sessionId) {
    activeSessionId.set(null);
  }
  await loadProjects();
}
```

**Step 2: Update backend close_session to clean up worktrees**

In `src-tauri/src/commands.rs`, update `close_session`:

```rust
#[tauri::command]
pub fn close_session(
    state: State<AppState>,
    project_id: String,
    session_id: String,
) -> Result<(), String> {
    let sess_id = Uuid::parse_str(&session_id).map_err(|e| e.to_string())?;
    let proj_id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    // Kill PTY
    let mut pty_mgr = state.pty_manager.lock().map_err(|e| e.to_string())?;
    pty_mgr.close_session(&sess_id)?;
    drop(pty_mgr);

    // Remove session from project and clean up worktree
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage.load_project(&proj_id)?;
    let session = project.sessions.iter().find(|s| s.id == sess_id).cloned();
    project.sessions.retain(|s| s.id != sess_id);
    storage.save_project(&project)?;

    // Clean up worktree if this was a refinement session
    if let Some(session) = session {
        if let Some(branch) = session.worktree_branch {
            let _ = crate::worktree::WorktreeManager::remove_worktree(
                &project.repo_path,
                &branch,
            );
        }
    }

    Ok(())
}
```

**Step 3: Add archive project UI**

In `src/lib/Sidebar.svelte`, add a context menu or button on project headers:

```typescript
async function archiveProject(projectId: string) {
  if (!confirm("Archive this project? All sessions will be closed.")) return;
  await invoke("archive_project", { projectId });
  await loadProjects();
}
```

**Step 4: Verify archive and close**

Run: `npm run tauri dev`
Test: Close a session → it disappears, worktree cleaned up. Archive a project → it disappears from sidebar.

**Step 5: Commit**

```bash
git add src/ src-tauri/
git commit -m "feat: add session close and project archive with worktree cleanup"
```

---

### Task 11: Error Handling and Polish

**Files:**
- Modify: `src/lib/Sidebar.svelte`
- Modify: `src/lib/Terminal.svelte`
- Create: `src/lib/Toast.svelte`

**Step 1: Create a simple toast notification component**

Create `src/lib/Toast.svelte`:

```svelte
<script lang="ts">
  import { writable } from "svelte/store";

  interface ToastMessage {
    id: number;
    text: string;
    type: "error" | "info";
  }

  export const toasts = writable<ToastMessage[]>([]);
  let counter = 0;

  export function showToast(text: string, type: "error" | "info" = "info") {
    const id = counter++;
    toasts.update((t) => [...t, { id, text, type }]);
    setTimeout(() => {
      toasts.update((t) => t.filter((msg) => msg.id !== id));
    }, 5000);
  }
</script>

<div class="toast-container">
  {#each $toasts as toast (toast.id)}
    <div class="toast" class:error={toast.type === "error"}>
      {toast.text}
    </div>
  {/each}
</div>

<style>
  .toast-container {
    position: fixed;
    bottom: 16px;
    right: 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 1000;
  }
  .toast {
    background: #313244;
    color: #cdd6f4;
    padding: 10px 16px;
    border-radius: 6px;
    font-size: 13px;
    max-width: 400px;
    border-left: 3px solid #89b4fa;
  }
  .toast.error {
    border-left-color: #f38ba8;
  }
</style>
```

**Step 2: Wrap all invoke calls in try/catch**

In `src/lib/Sidebar.svelte`, wrap each `invoke` call:

```typescript
import { showToast } from "./Toast.svelte";

async function createProject() {
  try {
    await invoke("create_project", {
      name: newProjectName,
      repoPath: newProjectPath,
    });
    // ... success handling
  } catch (e) {
    showToast(String(e), "error");
  }
}
```

Apply the same pattern to `createSession`, `createRefinement`, `closeSession`, `archiveProject`.

**Step 3: Add Toast to App.svelte**

```svelte
<script lang="ts">
  import Toast from "./lib/Toast.svelte";
</script>

<!-- At the end of the template -->
<Toast />
```

**Step 4: Verify error handling**

Run: `npm run tauri dev`
Test: Try creating a project with a non-existent path → toast shows error. Try creating a refinement with a branch name that already exists → toast shows git error.

**Step 5: Commit**

```bash
git add src/
git commit -m "feat: add toast notifications and error handling"
```

---

### Task 12: Load Existing Project Flow

**Files:**
- Modify: `src/lib/Sidebar.svelte`

**Step 1: Update the "New" button to show two options**

Replace the simple `showNewModal` toggle with a dropdown:

```svelte
{#if showNewModal}
  <div class="new-project-form">
    <div class="form-tabs">
      <button
        class:active={newMode === "create"}
        onclick={() => (newMode = "create")}
      >
        Create
      </button>
      <button
        class:active={newMode === "load"}
        onclick={() => (newMode = "load")}
      >
        Load Existing
      </button>
    </div>

    <input bind:value={newProjectName} placeholder="Project name" />
    <input bind:value={newProjectPath} placeholder="Repo path" />

    <div class="form-actions">
      <button onclick={newMode === "create" ? createProject : loadExistingProject}>
        {newMode === "create" ? "Create" : "Load"}
      </button>
      <button onclick={() => (showNewModal = false)}>Cancel</button>
    </div>
  </div>
{/if}
```

```typescript
let newMode = $state<"create" | "load">("create");

async function loadExistingProject() {
  try {
    await invoke("load_project", {
      name: newProjectName,
      repoPath: newProjectPath,
    });
    showNewModal = false;
    newProjectName = "";
    newProjectPath = "";
    await loadProjects();
  } catch (e) {
    showToast(String(e), "error");
  }
}
```

**Step 2: Verify load existing validates git repo**

Run: `npm run tauri dev`
Test: Try loading a non-git directory → error toast. Load a real git repo with an existing `agents.md` → project created, repo's `agents.md` is used.

**Step 3: Commit**

```bash
git add src/
git commit -m "feat: add load existing project flow with git validation"
```
