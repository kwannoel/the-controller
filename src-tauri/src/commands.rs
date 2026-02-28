use std::path::Path;

use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::models::{Project, SessionConfig};
use crate::state::AppState;
use crate::worktree::WorktreeManager;

const DEFAULT_AGENTS_MD: &str = r#"# Agents

## Default Agent

You are a helpful coding assistant working on this project.
"#;

#[tauri::command]
pub fn create_project(
    state: State<AppState>,
    name: String,
    repo_path: String,
) -> Result<Project, String> {
    let path = Path::new(&repo_path);
    if !path.is_dir() {
        return Err(format!("repo_path is not a directory: {}", repo_path));
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
    storage.save_project(&project).map_err(|e| e.to_string())?;

    // If repo doesn't have agents.md, create default one in config dir
    let repo_agents = path.join("agents.md");
    if !repo_agents.exists() {
        storage
            .save_agents_md(project.id, DEFAULT_AGENTS_MD)
            .map_err(|e| e.to_string())?;
    }

    Ok(project)
}

#[tauri::command]
pub fn load_project(
    state: State<AppState>,
    name: String,
    repo_path: String,
) -> Result<Project, String> {
    let path = Path::new(&repo_path);
    if !path.is_dir() {
        return Err(format!("repo_path is not a directory: {}", repo_path));
    }

    // Validate it's a git repo
    let git_dir = path.join(".git");
    if !git_dir.exists() {
        return Err(format!("not a git repository: {}", repo_path));
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
    storage.save_project(&project).map_err(|e| e.to_string())?;

    // Only create default agents.md if repo doesn't have one
    let repo_agents = path.join("agents.md");
    if !repo_agents.exists() {
        storage
            .save_agents_md(project.id, DEFAULT_AGENTS_MD)
            .map_err(|e| e.to_string())?;
    }

    Ok(project)
}

#[tauri::command]
pub fn list_projects(state: State<AppState>) -> Result<Vec<Project>, String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let projects = storage.list_projects().map_err(|e| e.to_string())?;
    let active: Vec<Project> = projects.into_iter().filter(|p| !p.archived).collect();
    Ok(active)
}

#[tauri::command]
pub fn archive_project(state: State<AppState>, project_id: String) -> Result<(), String> {
    let id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage.load_project(id).map_err(|e| e.to_string())?;
    project.archived = true;
    project.sessions.clear();
    storage.save_project(&project).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_agents_md(state: State<AppState>, project_id: String) -> Result<String, String> {
    let id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let project = storage.load_project(id).map_err(|e| e.to_string())?;
    storage.get_agents_md(&project).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_agents_md(
    state: State<AppState>,
    project_id: String,
    content: String,
) -> Result<(), String> {
    let id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    storage
        .save_agents_md(id, &content)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_session(
    state: State<AppState>,
    app_handle: AppHandle,
    project_id: String,
    label: String,
) -> Result<String, String> {
    let project_uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let session_id = Uuid::new_v4();

    // Load the project, add the session config, and save it back
    let repo_path = {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let mut project = storage.load_project(project_uuid).map_err(|e| e.to_string())?;

        let session_config = SessionConfig {
            id: session_id,
            label,
            worktree_path: None,
            worktree_branch: None,
        };
        project.sessions.push(session_config);
        storage.save_project(&project).map_err(|e| e.to_string())?;

        project.repo_path.clone()
    };

    // Spawn the PTY session in the project's repo directory
    let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
    pty_manager.spawn_session(session_id, &repo_path, app_handle)?;

    Ok(session_id.to_string())
}

#[tauri::command]
pub fn write_to_pty(
    state: State<AppState>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    let id = Uuid::parse_str(&session_id).map_err(|e| e.to_string())?;
    let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
    pty_manager.write_to_session(id, data.as_bytes())
}

#[tauri::command]
pub fn resize_pty(
    state: State<AppState>,
    session_id: String,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    let id = Uuid::parse_str(&session_id).map_err(|e| e.to_string())?;
    let pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
    pty_manager.resize_session(id, rows, cols)
}

#[tauri::command]
pub fn close_session(
    state: State<AppState>,
    project_id: String,
    session_id: String,
) -> Result<(), String> {
    let project_uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let session_uuid = Uuid::parse_str(&session_id).map_err(|e| e.to_string())?;

    // Close the PTY session (scoped to release lock before acquiring storage)
    {
        let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
        pty_manager.close_session(session_uuid)?;
    }

    // Remove session and clean up worktree
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage
        .load_project(project_uuid)
        .map_err(|e| e.to_string())?;

    // Find session before removing to check for worktree
    let session = project.sessions.iter().find(|s| s.id == session_uuid).cloned();
    project.sessions.retain(|s| s.id != session_uuid);
    storage.save_project(&project).map_err(|e| e.to_string())?;

    // Clean up worktree if this was a refinement session
    if let Some(session) = session {
        if let Some(branch) = session.worktree_branch {
            let _ = WorktreeManager::remove_worktree(&project.repo_path, &branch);
        }
    }

    Ok(())
}

#[tauri::command]
pub fn create_refinement(
    state: State<AppState>,
    app_handle: AppHandle,
    project_id: String,
    branch_name: String,
) -> Result<String, String> {
    let project_uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let session_id = Uuid::new_v4();

    // Load project to get repo_path
    let repo_path = {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let project = storage.load_project(project_uuid).map_err(|e| e.to_string())?;
        project.repo_path.clone()
    };

    // Create the worktree
    let worktree_path = WorktreeManager::create_worktree(&repo_path, &branch_name)?;
    let worktree_path_str = worktree_path
        .to_str()
        .ok_or_else(|| "worktree path is not valid UTF-8".to_string())?
        .to_string();

    // Create session config with worktree info, add to project, and save
    {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let mut project = storage.load_project(project_uuid).map_err(|e| e.to_string())?;

        let session_config = SessionConfig {
            id: session_id,
            label: branch_name.clone(),
            worktree_path: Some(worktree_path_str.clone()),
            worktree_branch: Some(branch_name),
        };
        project.sessions.push(session_config);
        storage.save_project(&project).map_err(|e| e.to_string())?;
    }

    // Spawn PTY session in the WORKTREE directory (not the main repo)
    let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
    pty_manager.spawn_session(session_id, &worktree_path_str, app_handle)?;

    Ok(session_id.to_string())
}
