use std::path::Path;

use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::config;
use crate::models::{CommitInfo, Project, SessionConfig};
use crate::state::AppState;
use crate::worktree::WorktreeManager;

mod github;
mod media;

/// Validate a project name. Rejects empty names, names containing `/` or `\`,
/// and names starting with `.`.
pub(crate) fn validate_project_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.starts_with('.') {
        return Err(format!("Invalid project name: {}", name));
    }
    Ok(())
}

/// Generate the next session label by finding the highest existing session number
/// and returning "session-N-<6-char-uuid>" where N = max + 1. The UUID suffix
/// guarantees uniqueness even when branches from deleted sessions persist on the
/// remote.
pub(crate) fn next_session_label(sessions: &[SessionConfig]) -> String {
    let max_num = sessions
        .iter()
        .filter_map(|s| s.label.strip_prefix("session-"))
        .filter_map(|n| n.split('-').next()?.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    let short_id = &Uuid::new_v4().to_string()[..6];
    format!("session-{}-{}", max_num + 1, short_id)
}

const DEFAULT_AGENTS_MD: &str = r#"# {name}

One-line project description.

## Task Workflow (CRITICAL)

For every new task, follow this workflow:

1. **File a GitHub issue** -- Create an issue describing the task before starting work.
2. **Update with design plan** -- Once the design is complete, update the GitHub issue with the design plan.
3. **Update with implementation plan** -- Once the implementation plan is ready, update the GitHub issue with it.
4. **Close the issue** -- Close the GitHub issue once the task is fully completed and verified.
5. **Update the merge commit/PR** -- After merging, update the merge note to summarize what work was done.

## Task Structure (CRITICAL -- NEVER SKIP)

**This is the most important rule. Every task, no matter how small, MUST follow this structure before writing any code. No exceptions.**

1. **Definition**: What's the task? Why are we doing it? How will we approach it?
2. **Constraints**: What are the design constraints -- from the user prompt, codebase conventions, or what can be inferred?
3. **Validation**: How do I know for sure it was implemented as expected? Can I enforce it with flexible and non-brittle tests? I must validate before I consider a task complete. For semantic changes (bug fixes, feature refinements): if I revert my implementation, the test must still fail. After the implementation, the test must pass.

**If you catch yourself writing code without having stated all three above, STOP and state them first.**

## Key Docs

- `docs/plans/` -- Design and implementation plans.

## Tech Stack

<!-- Fill in your project's tech stack -->

## Dev Commands

<!-- Fill in your project's dev commands -->
"#;

pub fn render_agents_md(name: &str) -> String {
    DEFAULT_AGENTS_MD.replace("{name}", name)
}

/// Run storage migrations on startup (worktree path format, etc.).
/// PTY connections are deferred to `connect_session` so each terminal
/// can attach at the correct size.
#[tauri::command]
pub fn restore_sessions(state: State<AppState>) -> Result<(), String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let projects = storage.list_projects().map_err(|e| e.to_string())?;
    // Migrate worktree paths from UUID-based to name-based directories
    for project in &projects {
        if let Err(e) = storage.migrate_worktree_paths(project) {
            eprintln!(
                "Failed to migrate worktrees for project '{}': {}",
                project.name, e
            );
        }
    }
    Ok(())
}

/// Connect a terminal to its PTY session at the given size.
/// Called by each Terminal component after it measures its dimensions.
/// No-op if the session is already connected.
#[tauri::command]
pub fn connect_session(
    state: State<AppState>,
    app_handle: AppHandle,
    session_id: String,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    let id = Uuid::parse_str(&session_id).map_err(|e| e.to_string())?;

    // Check if already connected
    {
        let pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
        if pty_manager.sessions.contains_key(&id) {
            return Ok(());
        }
    }

    // Find session config from storage
    let (session_dir, kind) = {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let projects = storage.list_projects().map_err(|e| e.to_string())?;
        projects
            .iter()
            .flat_map(|p| p.sessions.iter().map(move |s| (p, s)))
            .find(|(_, s)| s.id == id)
            .map(|(p, s)| {
                let dir = s
                    .worktree_path
                    .clone()
                    .unwrap_or_else(|| p.repo_path.clone());
                (dir, s.kind.clone())
            })
            .ok_or_else(|| format!("session not found: {}", session_id))?
    };

    let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
    pty_manager.spawn_session(id, &session_dir, &kind, app_handle, true, None, rows, cols)
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
            .save_agents_md(project.id, &render_agents_md(&project.name))
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
            .save_agents_md(project.id, &render_agents_md(&project.name))
            .map_err(|e| e.to_string())?;
    }

    Ok(project)
}

#[tauri::command]
pub fn list_projects(state: State<AppState>) -> Result<Vec<Project>, String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let projects = storage.list_projects().map_err(|e| e.to_string())?;
    Ok(projects.into_iter().filter(|p| !p.archived).collect())
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
                let _ = WorktreeManager::remove_worktree(wt_path, &project.repo_path, branch);
            }
        }
    }

    // Delete project metadata from ~/.the-controller/projects/{id}/
    storage.delete_project_dir(id).map_err(|e| e.to_string())?;

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
    Ok(projects.into_iter().filter(|p| p.archived).collect())
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
        pty_manager.spawn_session(
            session_id,
            &session_dir,
            &kind,
            app_handle.clone(),
            true,
            None,
            24,
            80,
        )?;
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
    github_issue: Option<crate::models::GithubIssue>,
    background: Option<bool>,
) -> Result<String, String> {
    let kind = kind.unwrap_or_else(|| "claude".to_string());
    let background = background.unwrap_or(false);
    let project_uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let session_id = Uuid::new_v4();

    // Load the project and generate session label
    let (repo_path, label, base_dir, project_name) = {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let project = storage
            .load_project(project_uuid)
            .map_err(|e| e.to_string())?;
        let label = next_session_label(&project.sessions);
        (
            project.repo_path.clone(),
            label,
            storage.base_dir(),
            project.name.clone(),
        )
    };

    // Create worktree under ~/.the-controller/worktrees/{project_name}/{label}/
    let worktree_dir = base_dir.join("worktrees").join(&project_name).join(&label);

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

    // Build initial prompt from GitHub issue context (if any)
    let initial_prompt = github_issue.as_ref().map(|issue| {
        crate::session_args::build_issue_prompt(issue.number, &issue.title, &issue.url, background)
    });

    // Save session config
    {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let mut project = storage
            .load_project(project_uuid)
            .map_err(|e| e.to_string())?;

        let session_config = SessionConfig {
            id: session_id,
            label: label.clone(),
            worktree_path: wt_path,
            worktree_branch: wt_branch,
            archived: false,
            kind: kind.clone(),
            github_issue,
            initial_prompt: initial_prompt.clone(),
            done_commits: vec![],
        };
        project.sessions.push(session_config);
        storage.save_project(&project).map_err(|e| e.to_string())?;
    }

    // Spawn the PTY session in the worktree (or repo) directory
    let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
    pty_manager.spawn_session(
        session_id,
        &session_dir,
        &kind,
        app_handle,
        false,
        initial_prompt.as_deref(),
        24,
        80,
    )?;

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
pub fn set_initial_prompt(
    state: State<AppState>,
    project_id: String,
    session_id: String,
    prompt: String,
) -> Result<(), String> {
    let project_uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let session_uuid = Uuid::parse_str(&session_id).map_err(|e| e.to_string())?;

    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage
        .load_project(project_uuid)
        .map_err(|e| e.to_string())?;

    if let Some(session) = project.sessions.iter_mut().find(|s| s.id == session_uuid) {
        if session.initial_prompt.is_none() {
            session.initial_prompt = Some(prompt);
            storage.save_project(&project).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
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
    let mut project = storage
        .load_project(project_uuid)
        .map_err(|e| e.to_string())?;

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
    let mut project = storage
        .load_project(project_uuid)
        .map_err(|e| e.to_string())?;

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
    pty_manager.spawn_session(
        session_uuid,
        &session_dir,
        &kind,
        app_handle,
        true,
        None,
        24,
        80,
    )?;

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

    let session = project
        .sessions
        .iter()
        .find(|s| s.id == session_uuid)
        .cloned();
    project.sessions.retain(|s| s.id != session_uuid);
    storage.save_project(&project).map_err(|e| e.to_string())?;

    // Optionally clean up worktree
    if delete_worktree {
        if let Some(session) = session {
            if let (Some(wt_path), Some(branch)) = (session.worktree_path, session.worktree_branch)
            {
                let _ = WorktreeManager::remove_worktree(&wt_path, &project.repo_path, &branch);
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub fn start_claude_login(state: State<AppState>, app_handle: AppHandle) -> Result<String, String> {
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
pub fn save_onboarding_config(state: State<AppState>, projects_root: String) -> Result<(), String> {
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
    let result = tokio::task::spawn_blocking(|| config::check_claude_cli_status())
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

    // Git init
    let repo = git2::Repository::init(&repo_path).map_err(|e| e.to_string())?;
    let sig = repo
        .signature()
        .unwrap_or_else(|_| git2::Signature::now("The Controller", "noreply@controller").unwrap());

    // Write template files to disk
    let agents_content = render_agents_md(&name);
    std::fs::write(repo_path.join("agents.md"), &agents_content)
        .map_err(|e| format!("failed to write agents.md: {}", e))?;
    let plans_dir = repo_path.join("docs").join("plans");
    std::fs::create_dir_all(&plans_dir)
        .map_err(|e| format!("failed to create docs/plans: {}", e))?;
    std::fs::write(plans_dir.join(".gitkeep"), "")
        .map_err(|e| format!("failed to write .gitkeep: {}", e))?;

    // Build git tree with template files
    let mut index = repo
        .index()
        .map_err(|e| format!("failed to get index: {}", e))?;
    index
        .add_path(std::path::Path::new("agents.md"))
        .map_err(|e| format!("failed to add agents.md to index: {}", e))?;
    index
        .add_path(std::path::Path::new("docs/plans/.gitkeep"))
        .map_err(|e| format!("failed to add .gitkeep to index: {}", e))?;
    index
        .write()
        .map_err(|e| format!("failed to write index: {}", e))?;
    let tree_id = index
        .write_tree()
        .map_err(|e| format!("failed to write tree: {}", e))?;
    let tree = repo
        .find_tree(tree_id)
        .map_err(|e| format!("failed to find tree: {}", e))?;

    repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
        .map_err(|e| format!("failed to create initial commit: {}", e))?;

    // Create GitHub remote — required for worktree push/PR workflows
    let gh_output = std::process::Command::new("gh")
        .args(["repo", "create", &name, "--private", "--source=.", "--push"])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| format!("Failed to run gh CLI: {}. Is gh installed?", e))?;
    if !gh_output.status.success() {
        let stderr = String::from_utf8_lossy(&gh_output.stderr);
        return Err(format!("Failed to create GitHub repo: {}", stderr.trim()));
    }

    // Configure repo to only allow squash merges
    let merge_cfg = std::process::Command::new("gh")
        .args([
            "api",
            &format!("repos/{{owner}}/{}", name),
            "-X", "PATCH",
            "-f", "allow_squash_merge=true",
            "-f", "allow_merge_commit=false",
            "-f", "allow_rebase_merge=false",
        ])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| format!("Failed to configure merge settings: {}", e))?;
    if !merge_cfg.status.success() {
        let stderr = String::from_utf8_lossy(&merge_cfg.stderr);
        return Err(format!("Failed to configure merge settings: {}", stderr.trim()));
    }

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

    Ok(project)
}

#[tauri::command]
pub async fn list_github_issues(
    repo_path: String,
    state: State<'_, AppState>,
) -> Result<Vec<crate::models::GithubIssue>, String> {
    github::list_github_issues(repo_path, state).await
}

#[tauri::command]
pub async fn generate_issue_body(title: String) -> Result<String, String> {
    github::generate_issue_body(title).await
}

#[tauri::command]
pub async fn create_github_issue(
    state: State<'_, AppState>,
    repo_path: String,
    title: String,
    body: String,
) -> Result<crate::models::GithubIssue, String> {
    github::create_github_issue(state, repo_path, title, body).await
}

#[tauri::command]
pub async fn post_github_comment(
    repo_path: String,
    issue_number: u64,
    body: String,
) -> Result<(), String> {
    github::post_github_comment(repo_path, issue_number, body).await
}

#[tauri::command]
pub async fn add_github_label(
    state: State<'_, AppState>,
    repo_path: String,
    issue_number: u64,
    label: String,
) -> Result<(), String> {
    github::add_github_label(state, repo_path, issue_number, label).await
}

#[tauri::command]
pub async fn remove_github_label(
    state: State<'_, AppState>,
    repo_path: String,
    issue_number: u64,
    label: String,
) -> Result<(), String> {
    github::remove_github_label(state, repo_path, issue_number, label).await
}

#[tauri::command]
pub async fn copy_image_file_to_clipboard(app: AppHandle, path: String) -> Result<(), String> {
    media::copy_image_file_to_clipboard(app, path).await
}

#[tauri::command]
pub async fn capture_app_screenshot(app: AppHandle) -> Result<String, String> {
    media::capture_app_screenshot(app).await
}

const MAX_MERGE_RETRIES: u32 = 5;
const REBASE_POLL_INTERVAL_SECS: u64 = 3;

#[tauri::command]
pub async fn merge_session_branch(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    project_id: String,
    session_id: String,
) -> Result<crate::models::MergeResponse, String> {
    let project_uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let session_uuid = Uuid::parse_str(&session_id).map_err(|e| e.to_string())?;

    let (repo_path, worktree_path, branch_name) = {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let project = storage
            .load_project(project_uuid)
            .map_err(|e| e.to_string())?;
        let session = project
            .sessions
            .iter()
            .find(|s| s.id == session_uuid)
            .ok_or_else(|| "Session not found".to_string())?;
        let wt_path = session
            .worktree_path
            .clone()
            .ok_or_else(|| "Session has no worktree".to_string())?;
        let branch = session
            .worktree_branch
            .clone()
            .ok_or_else(|| "Session has no branch".to_string())?;
        (project.repo_path.clone(), wt_path, branch)
    };

    for attempt in 0..MAX_MERGE_RETRIES {
        let rp = repo_path.clone();
        let wt = worktree_path.clone();
        let br = branch_name.clone();

        let result = tokio::task::spawn_blocking(move || {
            if WorktreeManager::is_rebase_in_progress(&wt) {
                // Rebase still in progress from a previous attempt — wait
                Ok(crate::worktree::MergeResult::RebaseConflicts)
            } else {
                WorktreeManager::merge_via_pr(&rp, &wt, &br)
            }
        })
        .await
        .map_err(|e| format!("Task failed: {}", e))??;

        match result {
            crate::worktree::MergeResult::PrCreated(url) => {
                return Ok(crate::models::MergeResponse::PrCreated { url });
            }
            crate::worktree::MergeResult::RebaseConflicts => {
                // Send a prompt to Claude to resolve conflicts
                let prompt = "merge\r";
                {
                    let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
                    let _ = pty_manager.write_to_session(session_uuid, prompt.as_bytes());
                }

                // Emit status event so frontend can show progress
                let _ = app_handle.emit(
                    "merge-status",
                    format!(
                        "Rebase conflicts (attempt {}/{}). Claude is resolving...",
                        attempt + 1,
                        MAX_MERGE_RETRIES
                    ),
                );

                // Poll until rebase is no longer in progress
                let wt_poll = worktree_path.clone();
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(REBASE_POLL_INTERVAL_SECS))
                        .await;
                    let wt_check = wt_poll.clone();
                    let still_rebasing = tokio::task::spawn_blocking(move || {
                        WorktreeManager::is_rebase_in_progress(&wt_check)
                    })
                    .await
                    .map_err(|e| format!("Task failed: {}", e))?;
                    if !still_rebasing {
                        break;
                    }
                }

                // Loop back — will sync main and rebase again
                continue;
            }
        }
    }

    Err(format!(
        "Merge failed after {} attempts due to recurring conflicts",
        MAX_MERGE_RETRIES
    ))
}

#[tauri::command]
pub fn get_session_commits(
    state: State<AppState>,
    project_id: String,
    session_id: String,
) -> Result<Vec<CommitInfo>, String> {
    let project_uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let session_uuid = Uuid::parse_str(&session_id).map_err(|e| e.to_string())?;

    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let project = storage
        .load_project(project_uuid)
        .map_err(|e| e.to_string())?;

    let session = project
        .sessions
        .iter()
        .find(|s| s.id == session_uuid)
        .ok_or_else(|| "Session not found".to_string())?;

    let worktree_path = match &session.worktree_path {
        Some(p) => p.clone(),
        None => return Ok(session.done_commits.clone()),
    };

    // Discover new commits on the branch that aren't on main
    let new_commits = discover_branch_commits(&worktree_path).unwrap_or_default();

    // Merge with previously stored commits (new first, then stored, dedup by hash)
    let mut seen = std::collections::HashSet::new();
    let mut all_commits = Vec::new();
    for c in new_commits.iter().chain(session.done_commits.iter()) {
        if seen.insert(c.hash.clone()) {
            all_commits.push(c.clone());
        }
    }

    // Persist if we found new commits
    if all_commits.len() > session.done_commits.len() {
        let mut project = project.clone();
        if let Some(s) = project.sessions.iter_mut().find(|s| s.id == session_uuid) {
            s.done_commits = all_commits.clone();
        }
        let _ = storage.save_project(&project);
    }

    Ok(all_commits)
}

/// Walk commits on the worktree branch that aren't on the main branch.
fn discover_branch_commits(worktree_path: &str) -> Result<Vec<CommitInfo>, String> {
    let repo = git2::Repository::discover(worktree_path)
        .map_err(|e| format!("Failed to open repo: {e}"))?;

    let head = repo.head().map_err(|e| format!("No HEAD: {e}"))?;
    let head_commit = head.peel_to_commit().map_err(|e| e.to_string())?;

    let main_oid = find_main_branch_oid(&repo);

    let mut revwalk = repo.revwalk().map_err(|e| e.to_string())?;
    revwalk.push(head_commit.id()).map_err(|e| e.to_string())?;
    revwalk
        .set_sorting(git2::Sort::TOPOLOGICAL)
        .map_err(|e| e.to_string())?;

    let mut commits = Vec::new();
    for oid in revwalk {
        let oid = oid.map_err(|e| e.to_string())?;
        if let Some(main) = main_oid {
            if oid == main {
                break;
            }
            if let Ok(base) = repo.merge_base(oid, main) {
                if base == oid {
                    break;
                }
            }
        }
        let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;
        let message = commit.summary().unwrap_or("").to_string();
        if message.starts_with("Initial commit") {
            continue;
        }
        let hash = format!("{}", &oid.to_string()[..7]);
        commits.push(CommitInfo { hash, message });
        if commits.len() >= 20 {
            break;
        }
    }

    Ok(commits)
}

fn find_main_branch_oid(repo: &git2::Repository) -> Option<git2::Oid> {
    for name in &["refs/heads/master", "refs/heads/main"] {
        if let Ok(reference) = repo.find_reference(name) {
            if let Ok(commit) = reference.peel_to_commit() {
                return Some(commit.id());
            }
        }
    }
    None
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
        let label = next_session_label(&sessions);
        assert!(label.starts_with("session-1-"), "expected session-1-<id>, got {}", label);
        assert_eq!(label.len(), "session-1-".len() + 6); // 6-char UUID suffix
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
                github_issue: None,
                initial_prompt: None,
                done_commits: vec![],
            },
            SessionConfig {
                id: Uuid::new_v4(),
                label: "session-2".to_string(),
                worktree_path: None,
                worktree_branch: None,
                archived: false,
                kind: "claude".to_string(),
                github_issue: None,
                initial_prompt: None,
                done_commits: vec![],
            },
        ];
        let label = next_session_label(&sessions);
        assert!(label.starts_with("session-3-"), "expected session-3-<id>, got {}", label);
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
                github_issue: None,
                initial_prompt: None,
                done_commits: vec![],
            },
            SessionConfig {
                id: Uuid::new_v4(),
                label: "session-2".to_string(),
                worktree_path: Some("/tmp/wt2".to_string()),
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
                worktree_path: Some("/tmp/wt3".to_string()),
                worktree_branch: Some("session-3".to_string()),
                archived: true,
                kind: "claude".to_string(),
                github_issue: None,
                initial_prompt: None,
                done_commits: vec![],
            },
        ];
        // Max is session-3, so next is session-4
        let label = next_session_label(&sessions);
        assert!(label.starts_with("session-4-"), "expected session-4-<id>, got {}", label);
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
            github_issue: None,
            initial_prompt: None,
            done_commits: vec![],
        }];
        let label = next_session_label(&sessions);
        assert!(label.starts_with("session-4-"), "expected session-4-<id>, got {}", label);
    }

    #[test]
    fn test_next_session_label_parses_new_format() {
        // Labels with UUID suffix (session-2-abc123) should parse correctly
        let sessions = vec![SessionConfig {
            id: Uuid::new_v4(),
            label: "session-2-a1b2c3".to_string(),
            worktree_path: None,
            worktree_branch: None,
            archived: false,
            kind: "claude".to_string(),
            github_issue: None,
            initial_prompt: None,
            done_commits: vec![],
        }];
        let label = next_session_label(&sessions);
        assert!(label.starts_with("session-3-"), "expected session-3-<id>, got {}", label);
    }

    #[test]
    fn test_next_session_label_unique() {
        let sessions: Vec<SessionConfig> = vec![];
        let label1 = next_session_label(&sessions);
        let label2 = next_session_label(&sessions);
        assert_ne!(label1, label2, "labels should be unique across calls");
    }

    // --- render_agents_md tests ---

    #[test]
    fn test_render_agents_md_replaces_name() {
        let result = render_agents_md("my-project");
        assert!(result.starts_with("# my-project"));
        assert!(!result.contains("{name}"));
    }

    #[test]
    fn test_render_agents_md_empty_name() {
        let result = render_agents_md("");
        assert!(result.starts_with("# \n"));
    }

    // --- find_main_branch_oid tests ---

    #[test]
    fn test_find_main_branch_oid_with_main_branch() {
        let tmp = tempfile::TempDir::new().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let tree_id = repo.treebuilder(None).unwrap().write().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
            .unwrap();

        // After init + commit, HEAD points to a branch — find_main_branch_oid should find it
        let oid = find_main_branch_oid(&repo);
        assert!(oid.is_some());
    }

    #[test]
    fn test_find_main_branch_oid_no_main_or_master() {
        let tmp = tempfile::TempDir::new().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let tree_id = repo.treebuilder(None).unwrap().write().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let oid = repo
            .commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
            .unwrap();

        // Rename the default branch to something other than main/master
        let commit = repo.find_commit(oid).unwrap();
        repo.branch("develop", &commit, false).unwrap();
        // Delete the original branch by setting HEAD to develop
        repo.set_head("refs/heads/develop").unwrap();
        if let Ok(mut branch) = repo.find_branch("main", git2::BranchType::Local) {
            let _ = branch.delete();
        }
        if let Ok(mut branch) = repo.find_branch("master", git2::BranchType::Local) {
            let _ = branch.delete();
        }

        let result = find_main_branch_oid(&repo);
        assert!(result.is_none());
    }

    // --- CommitInfo serialization ---

    #[test]
    fn test_commit_info_serialization() {
        let info = CommitInfo {
            hash: "abc1234".to_string(),
            message: "fix: resolve bug".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["hash"], "abc1234");
        assert_eq!(parsed["message"], "fix: resolve bug");
    }

    // --- additional validate_project_name edge cases ---

    #[test]
    fn test_project_name_with_spaces_is_valid() {
        // Spaces are not explicitly rejected
        assert!(validate_project_name("my project").is_ok());
    }

    #[test]
    fn test_project_name_with_hyphens_and_underscores() {
        assert!(validate_project_name("my-cool_project-2").is_ok());
    }

    #[test]
    fn test_project_name_single_char() {
        assert!(validate_project_name("a").is_ok());
    }
}
