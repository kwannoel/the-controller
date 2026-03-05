use std::path::Path;

use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::config;
use crate::models::{Project, SessionConfig};
use crate::state::AppState;
use crate::worktree::WorktreeManager;

/// Validate a project name. Rejects empty names, names containing `/` or `\`,
/// and names starting with `.`.
pub(crate) fn validate_project_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.starts_with('.')
    {
        return Err(format!("Invalid project name: {}", name));
    }
    Ok(())
}

/// Generate the next session label by finding the highest existing session number
/// and returning "session-N" where N = max + 1. This avoids collisions when
/// sessions are deleted or archived out of order.
pub(crate) fn next_session_label(sessions: &[SessionConfig]) -> String {
    let max_num = sessions
        .iter()
        .filter_map(|s| s.label.strip_prefix("session-"))
        .filter_map(|n| n.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    format!("session-{}", max_num + 1)
}

const DEFAULT_AGENTS_MD: &str = r#"# Agents

## Default Agent

You are a helpful coding assistant working on this project.
"#;

/// Re-spawn PTY sessions for all active (non-archived) sessions across all projects.
/// PTY processes don't survive restart, but session metadata and worktrees persist.
#[tauri::command]
pub fn restore_sessions(
    state: State<AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
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

    let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;

    for project in &projects {
        for session in &project.sessions {
            if session.archived {
                continue;
            }
            let session_dir = session
                .worktree_path
                .clone()
                .unwrap_or_else(|| project.repo_path.clone());

            if let Err(e) = pty_manager.spawn_session(session.id, &session_dir, &session.kind, app_handle.clone(), true)
            {
                eprintln!(
                    "Failed to restore session {} ({}): {}",
                    session.label, session.id, e
                );
            }
        }
    }

    Ok(())
}

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

    let storage = state.storage.lock().map_err(|e| e.to_string())?;

    // Reject duplicate project names
    if let Ok(existing) = storage.list_projects() {
        if existing.iter().any(|p| p.name == name) {
            return Err(format!("A project named '{}' already exists", name));
        }
    }

    let project = Project {
        id: Uuid::new_v4(),
        name,
        repo_path: repo_path.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        archived: false,
        sessions: vec![],
    };

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

    let storage = state.storage.lock().map_err(|e| e.to_string())?;

    // Return existing project if one with the same repo_path exists
    if let Ok(existing) = storage.list_projects() {
        if let Some(project) = existing.iter().find(|p| p.repo_path == repo_path) {
            return Ok(project.clone());
        }
        // Reject duplicate project names when creating new
        if existing.iter().any(|p| p.name == name) {
            return Err(format!("A project named '{}' already exists", name));
        }
    }

    let project = Project {
        id: Uuid::new_v4(),
        name,
        repo_path: repo_path.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        archived: false,
        sessions: vec![],
    };

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
    Ok(projects
        .into_iter()
        .filter(|p| !p.archived)
        .collect())
}

#[tauri::command]
pub fn archive_project(state: State<AppState>, project_id: String) -> Result<(), String> {
    let id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage.load_project(id).map_err(|e| e.to_string())?;

    project.archived = true;

    // Close PTYs for all active sessions, mark them archived (keep worktrees)
    {
        let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
        for session in &mut project.sessions {
            if !session.archived {
                let _ = pty_manager.close_session(session.id);
                session.archived = true;
            }
        }
    }

    storage.save_project(&project).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_project(
    state: State<AppState>,
    project_id: String,
    delete_repo: bool,
) -> Result<(), String> {
    let id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let project = storage.load_project(id).map_err(|e| e.to_string())?;

    // Close all PTY sessions and clean up worktrees
    {
        let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
        for session in &project.sessions {
            let _ = pty_manager.close_session(session.id);
            if let (Some(wt_path), Some(branch)) =
                (&session.worktree_path, &session.worktree_branch)
            {
                let _ =
                    WorktreeManager::remove_worktree(wt_path, &project.repo_path, branch);
            }
        }
    }

    // Delete project metadata from ~/.the-controller/projects/{id}/
    storage
        .delete_project_dir(id)
        .map_err(|e| e.to_string())?;

    // Optionally delete the repo directory
    if delete_repo {
        if Path::new(&project.repo_path).exists() {
            std::fs::remove_dir_all(&project.repo_path)
                .map_err(|e| format!("failed to delete repo: {}", e))?;
        }
    }

    Ok(())
}

#[tauri::command]
pub fn list_archived_projects(state: State<AppState>) -> Result<Vec<Project>, String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let projects = storage.list_projects().map_err(|e| e.to_string())?;
    Ok(projects
        .into_iter()
        .filter(|p| p.archived)
        .collect())
}

#[tauri::command]
pub fn unarchive_project(
    state: State<AppState>,
    app_handle: AppHandle,
    project_id: String,
) -> Result<(), String> {
    let id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage.load_project(id).map_err(|e| e.to_string())?;

    // Collect sessions to restore before taking pty_manager lock
    let to_restore: Vec<(Uuid, String, String)> = project
        .sessions
        .iter()
        .filter(|s| s.archived)
        .map(|s| {
            let dir = s
                .worktree_path
                .clone()
                .unwrap_or_else(|| project.repo_path.clone());
            (s.id, dir, s.kind.clone())
        })
        .collect();

    project.archived = false;

    for session in &mut project.sessions {
        if session.archived {
            session.archived = false;
        }
    }
    storage.save_project(&project).map_err(|e| e.to_string())?;
    drop(storage);

    // Spawn PTYs for restored sessions
    let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
    for (session_id, session_dir, kind) in to_restore {
        pty_manager.spawn_session(session_id, &session_dir, &kind, app_handle.clone(), true)?;
    }

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
    kind: Option<String>,
) -> Result<String, String> {
    let kind = kind.unwrap_or_else(|| "claude".to_string());
    let project_uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let session_id = Uuid::new_v4();

    // Load the project and generate session label
    let (repo_path, label, base_dir, project_name) = {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let project = storage.load_project(project_uuid).map_err(|e| e.to_string())?;
        let label = next_session_label(&project.sessions);
        (project.repo_path.clone(), label, storage.base_dir(), project.name.clone())
    };

    // Create worktree under ~/.the-controller/worktrees/{project_name}/{label}/
    let worktree_dir = base_dir
        .join("worktrees")
        .join(&project_name)
        .join(&label);

    // Try to create a worktree; fall back to repo path for repos without commits
    let (session_dir, wt_path, wt_branch) =
        match WorktreeManager::create_worktree(&repo_path, &label, &worktree_dir) {
            Ok(worktree_path) => {
                let wt_str = worktree_path
                    .to_str()
                    .ok_or_else(|| "worktree path is not valid UTF-8".to_string())?
                    .to_string();
                (wt_str.clone(), Some(wt_str), Some(label.clone()))
            }
            Err(e) if e == "unborn_branch" => {
                // Repo has no commits — use repo path directly, no worktree
                (repo_path.clone(), None, None)
            }
            Err(e) => return Err(e),
        };

    // Save session config
    {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let mut project = storage.load_project(project_uuid).map_err(|e| e.to_string())?;

        let session_config = SessionConfig {
            id: session_id,
            label: label.clone(),
            worktree_path: wt_path,
            worktree_branch: wt_branch,
            archived: false,
            kind: kind.clone(),
        };
        project.sessions.push(session_config);
        storage.save_project(&project).map_err(|e| e.to_string())?;
    }

    // Spawn the PTY session in the worktree (or repo) directory
    let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
    pty_manager.spawn_session(session_id, &session_dir, &kind, app_handle, false)?;

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
pub fn send_raw_to_pty(
    state: State<AppState>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    let id = Uuid::parse_str(&session_id).map_err(|e| e.to_string())?;
    let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
    pty_manager.send_raw_to_session(id, data.as_bytes())
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
pub fn archive_session(
    state: State<AppState>,
    project_id: String,
    session_id: String,
) -> Result<(), String> {
    let project_uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let session_uuid = Uuid::parse_str(&session_id).map_err(|e| e.to_string())?;

    // Close the PTY session and kill tmux (worktree stays on disk)
    {
        let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
        let _ = pty_manager.close_session(session_uuid);
    }

    // Mark session as archived — keep worktree path/branch intact
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage.load_project(project_uuid).map_err(|e| e.to_string())?;

    if let Some(session) = project.sessions.iter_mut().find(|s| s.id == session_uuid) {
        session.archived = true;
    } else {
        return Err("Session not found".to_string());
    }

    storage.save_project(&project).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn unarchive_session(
    state: State<AppState>,
    app_handle: AppHandle,
    project_id: String,
    session_id: String,
) -> Result<(), String> {
    let project_uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let session_uuid = Uuid::parse_str(&session_id).map_err(|e| e.to_string())?;

    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage.load_project(project_uuid).map_err(|e| e.to_string())?;

    let session = project
        .sessions
        .iter_mut()
        .find(|s| s.id == session_uuid && s.archived)
        .ok_or_else(|| "Archived session not found".to_string())?;

    // Use existing worktree path, or fall back to repo path
    let session_dir = session
        .worktree_path
        .clone()
        .unwrap_or_else(|| project.repo_path.clone());
    let kind = session.kind.clone();

    session.archived = false;
    storage.save_project(&project).map_err(|e| e.to_string())?;

    // Need to drop storage lock before acquiring pty_manager lock
    drop(storage);

    // Spawn the PTY session in the existing worktree directory
    let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
    pty_manager.spawn_session(session_uuid, &session_dir, &kind, app_handle, true)?;

    Ok(())
}

#[tauri::command]
pub fn close_session(
    state: State<AppState>,
    project_id: String,
    session_id: String,
    delete_worktree: bool,
) -> Result<(), String> {
    let project_uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let session_uuid = Uuid::parse_str(&session_id).map_err(|e| e.to_string())?;

    // Try to close the PTY session (may not exist for archived sessions)
    {
        let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
        let _ = pty_manager.close_session(session_uuid);
    }

    // Remove session from project
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage
        .load_project(project_uuid)
        .map_err(|e| e.to_string())?;

    let session = project.sessions.iter().find(|s| s.id == session_uuid).cloned();
    project.sessions.retain(|s| s.id != session_uuid);
    storage.save_project(&project).map_err(|e| e.to_string())?;

    // Optionally clean up worktree
    if delete_worktree {
        if let Some(session) = session {
            if let (Some(wt_path), Some(branch)) =
                (session.worktree_path, session.worktree_branch)
            {
                let _ =
                    WorktreeManager::remove_worktree(&wt_path, &project.repo_path, &branch);
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub fn start_claude_login(
    state: State<AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    let session_id = Uuid::new_v4();
    let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
    pty_manager.spawn_command(session_id, "claude", &["login"], app_handle)?;
    Ok(session_id.to_string())
}

#[tauri::command]
pub fn stop_claude_login(state: State<AppState>, session_id: String) -> Result<(), String> {
    let id = Uuid::parse_str(&session_id).map_err(|e| e.to_string())?;
    let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
    pty_manager.close_session(id)
}

#[tauri::command]
pub fn home_dir() -> Result<String, String> {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "Could not determine home directory".to_string())
}

#[tauri::command]
pub fn check_onboarding(state: State<AppState>) -> Result<Option<config::Config>, String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let base_dir = storage.base_dir();
    Ok(config::load_config(&base_dir))
}

#[tauri::command]
pub fn save_onboarding_config(
    state: State<AppState>,
    projects_root: String,
) -> Result<(), String> {
    let path = Path::new(&projects_root);
    if !path.is_dir() {
        return Err(format!(
            "projects_root is not an existing directory: {}",
            projects_root
        ));
    }

    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let base_dir = storage.base_dir();
    let cfg = config::Config { projects_root };
    config::save_config(&base_dir, &cfg).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_claude_cli() -> Result<String, String> {
    let result =
        tokio::task::spawn_blocking(|| config::check_claude_cli_status())
            .await
            .map_err(|e| format!("Task failed: {}", e))?;
    Ok(result)
}

#[tauri::command]
pub fn list_directories_at(path: String) -> Result<Vec<config::DirEntry>, String> {
    let p = Path::new(&path);
    if !p.is_dir() {
        return Err(format!("Not a directory: {}", path));
    }
    config::list_directories(p).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_root_directories(state: State<AppState>) -> Result<Vec<config::DirEntry>, String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let base_dir = storage.base_dir();
    let cfg = config::load_config(&base_dir)
        .ok_or_else(|| "No config found. Complete onboarding first.".to_string())?;
    config::list_directories(Path::new(&cfg.projects_root)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn generate_project_names(description: String) -> Result<Vec<String>, String> {
    config::generate_names_via_cli(&description)
}

#[tauri::command]
pub fn scaffold_project(state: State<AppState>, name: String) -> Result<Project, String> {
    validate_project_name(&name)?;

    let storage = state.storage.lock().map_err(|e| e.to_string())?;

    // Reject duplicate project names
    if let Ok(existing) = storage.list_projects() {
        if existing.iter().any(|p| p.name == name) {
            return Err(format!("A project named '{}' already exists", name));
        }
    }

    let cfg = config::load_config(&storage.base_dir())
        .ok_or_else(|| "No config found. Complete onboarding first.".to_string())?;

    let repo_path = std::path::Path::new(&cfg.projects_root).join(&name);
    if repo_path.exists() {
        return Err(format!("Directory already exists: {}", name));
    }

    // Create directory
    std::fs::create_dir_all(&repo_path).map_err(|e| e.to_string())?;

    // Git init + initial commit so worktrees can be created
    let repo = git2::Repository::init(&repo_path).map_err(|e| e.to_string())?;
    let sig = repo
        .signature()
        .unwrap_or_else(|_| git2::Signature::now("The Controller", "noreply@controller").unwrap());
    let tree_id = repo
        .treebuilder(None)
        .and_then(|tb| tb.write())
        .map_err(|e| format!("failed to create initial tree: {}", e))?;
    let tree = repo
        .find_tree(tree_id)
        .map_err(|e| format!("failed to find tree: {}", e))?;
    repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
        .map_err(|e| format!("failed to create initial commit: {}", e))?;

    // Create project entry
    let project = Project {
        id: Uuid::new_v4(),
        name,
        repo_path: repo_path.to_string_lossy().to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        archived: false,
        sessions: vec![],
    };
    storage.save_project(&project).map_err(|e| e.to_string())?;

    // Create default agents.md
    storage
        .save_agents_md(project.id, DEFAULT_AGENTS_MD)
        .map_err(|e| e.to_string())?;

    Ok(project)
}

/// Parse a GitHub remote URL into an "owner/repo" string.
/// Handles SSH (git@github.com:owner/repo.git), HTTPS, and HTTP URLs.
pub(crate) fn parse_github_nwo(url: &str) -> Result<String, String> {
    // SSH: git@github.com:owner/repo.git
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        return Ok(rest.trim_end_matches(".git").to_string());
    }
    // HTTPS/HTTP: https://github.com/owner/repo.git
    if let Some(rest) = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
    {
        return Ok(rest.trim_end_matches(".git").to_string());
    }

    Err(format!("Not a GitHub remote URL: {}", url))
}

/// Extract the GitHub owner/repo from a local git repository's origin remote.
/// Handles both SSH (git@github.com:owner/repo.git) and HTTPS (https://github.com/owner/repo.git) URLs.
fn extract_github_repo(repo_path: &str) -> Result<String, String> {
    let repo = git2::Repository::discover(repo_path)
        .map_err(|e| format!("Failed to open repo: {}", e))?;
    let remote = repo
        .find_remote("origin")
        .map_err(|_| "No 'origin' remote found".to_string())?;
    let url = remote
        .url()
        .ok_or_else(|| "Origin remote URL is not valid UTF-8".to_string())?;

    parse_github_nwo(url)
}

#[tauri::command]
pub async fn list_github_issues(repo_path: String) -> Result<Vec<crate::models::GithubIssue>, String> {
    let repo_path_clone = repo_path.clone();
    let nwo = tokio::task::spawn_blocking(move || extract_github_repo(&repo_path_clone))
        .await
        .map_err(|e| format!("Task failed: {}", e))??;

    let output = tokio::process::Command::new("gh")
        .args([
            "issue", "list",
            "--repo", &nwo,
            "--json", "number,title,url,labels",
            "--limit", "50",
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to run gh: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh issue list failed: {}", stderr));
    }

    let issues: Vec<crate::models::GithubIssue> =
        serde_json::from_slice(&output.stdout)
            .map_err(|e| format!("Failed to parse gh output: {}", e))?;

    Ok(issues)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SessionConfig;
    use uuid::Uuid;

    // --- validate_project_name tests ---

    #[test]
    fn test_valid_project_name() {
        assert!(validate_project_name("my-cool-project").is_ok());
    }

    #[test]
    fn test_empty_project_name() {
        let result = validate_project_name("");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid project name"));
    }

    #[test]
    fn test_project_name_with_forward_slash() {
        let result = validate_project_name("foo/bar");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid project name"));
    }

    #[test]
    fn test_project_name_with_backslash() {
        let result = validate_project_name("foo\\bar");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid project name"));
    }

    #[test]
    fn test_project_name_starting_with_dot() {
        let result = validate_project_name(".hidden");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid project name"));
    }

    // --- next_session_label tests ---

    #[test]
    fn test_next_session_label_empty() {
        let sessions: Vec<SessionConfig> = vec![];
        assert_eq!(next_session_label(&sessions), "session-1");
    }

    #[test]
    fn test_next_session_label_two_existing() {
        let sessions = vec![
            SessionConfig {
                id: Uuid::new_v4(),
                label: "session-1".to_string(),
                worktree_path: None,
                worktree_branch: None,
                archived: false,
                kind: "claude".to_string(),
            },
            SessionConfig {
                id: Uuid::new_v4(),
                label: "session-2".to_string(),
                worktree_path: None,
                worktree_branch: None,
                archived: false,
                kind: "claude".to_string(),
            },
        ];
        assert_eq!(next_session_label(&sessions), "session-3");
    }

    #[test]
    fn test_next_session_label_counts_all_sessions() {
        // Archived sessions still occupy worktree branches, so they must be counted
        let sessions = vec![
            SessionConfig {
                id: Uuid::new_v4(),
                label: "session-1".to_string(),
                worktree_path: Some("/tmp/wt1".to_string()),
                worktree_branch: Some("session-1".to_string()),
                archived: true,
                kind: "claude".to_string(),
            },
            SessionConfig {
                id: Uuid::new_v4(),
                label: "session-2".to_string(),
                worktree_path: Some("/tmp/wt2".to_string()),
                worktree_branch: Some("session-2".to_string()),
                archived: false,
                kind: "claude".to_string(),
            },
            SessionConfig {
                id: Uuid::new_v4(),
                label: "session-3".to_string(),
                worktree_path: Some("/tmp/wt3".to_string()),
                worktree_branch: Some("session-3".to_string()),
                archived: true,
                kind: "claude".to_string(),
            },
        ];
        // Max is session-3, so next is session-4
        assert_eq!(next_session_label(&sessions), "session-4");
    }

    #[test]
    fn test_next_session_label_with_gap() {
        // Only session-3 remains (1 and 2 deleted). Next should be session-4, not session-2.
        let sessions = vec![SessionConfig {
            id: Uuid::new_v4(),
            label: "session-3".to_string(),
            worktree_path: None,
            worktree_branch: None,
            archived: false,
            kind: "claude".to_string(),
        }];
        assert_eq!(next_session_label(&sessions), "session-4");
    }

    // --- parse_github_nwo tests ---

    #[test]
    fn test_parse_github_nwo_ssh() {
        assert_eq!(
            parse_github_nwo("git@github.com:owner/repo.git").unwrap(),
            "owner/repo"
        );
    }

    #[test]
    fn test_parse_github_nwo_https() {
        assert_eq!(
            parse_github_nwo("https://github.com/owner/repo.git").unwrap(),
            "owner/repo"
        );
    }

    #[test]
    fn test_parse_github_nwo_https_no_git_suffix() {
        assert_eq!(
            parse_github_nwo("https://github.com/owner/repo").unwrap(),
            "owner/repo"
        );
    }

    #[test]
    fn test_parse_github_nwo_http() {
        assert_eq!(
            parse_github_nwo("http://github.com/owner/repo.git").unwrap(),
            "owner/repo"
        );
    }

    #[test]
    fn test_parse_github_nwo_non_github_url() {
        let result = parse_github_nwo("https://gitlab.com/owner/repo.git");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Not a GitHub remote URL"));
    }
}
