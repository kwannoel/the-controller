use std::path::Path;

use tauri::{AppHandle, Emitter, State};
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

            if let Err(e) = pty_manager.spawn_session(session.id, &session_dir, &session.kind, app_handle.clone(), true, None)
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
        pty_manager.spawn_session(session_id, &session_dir, &kind, app_handle.clone(), true, None)?;
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

    // Sync main branch before creating worktree so session starts from latest
    if let Err(e) = WorktreeManager::sync_main(&repo_path) {
        eprintln!("Warning: failed to sync main branch: {}", e);
    }

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

    // Build initial prompt from GitHub issue context (if any)
    let initial_prompt = github_issue.as_ref().map(|issue| {
        format!(
            "You are working on GitHub issue #{}: {}\nIssue URL: {}\nPlease include 'closes #{}' in any PR descriptions or final commit messages.",
            issue.number, issue.title, issue.url, issue.number
        )
    });

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
            github_issue,
        };
        project.sessions.push(session_config);
        storage.save_project(&project).map_err(|e| e.to_string())?;
    }

    // Spawn the PTY session in the worktree (or repo) directory
    let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
    pty_manager.spawn_session(session_id, &session_dir, &kind, app_handle, false, initial_prompt.as_deref())?;

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
    pty_manager.spawn_session(session_uuid, &session_dir, &kind, app_handle, true, None)?;

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
    let mut index = repo.index().map_err(|e| format!("failed to get index: {}", e))?;
    index
        .add_path(std::path::Path::new("agents.md"))
        .map_err(|e| format!("failed to add agents.md to index: {}", e))?;
    index
        .add_path(std::path::Path::new("docs/plans/.gitkeep"))
        .map_err(|e| format!("failed to add .gitkeep to index: {}", e))?;
    index.write().map_err(|e| format!("failed to write index: {}", e))?;
    let tree_id = index
        .write_tree()
        .map_err(|e| format!("failed to write tree: {}", e))?;
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

/// Parse a GitHub issue URL like "https://github.com/owner/repo/issues/42" and return the issue number.
fn parse_github_issue_url(url: &str) -> Result<u64, String> {
    let url = url.trim();
    let parts: Vec<&str> = url.rsplitn(2, '/').collect();
    if parts.len() == 2 {
        if let Ok(num) = parts[0].parse::<u64>() {
            return Ok(num);
        }
    }
    Err(format!("Could not parse issue number from URL: {}", url))
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

#[tauri::command]
pub async fn generate_issue_body(title: String) -> Result<String, String> {
    let prompt = format!(
        "Write a concise GitHub issue body for an issue titled: \"{}\". \
         Include a Summary section and a Details section. \
         Keep it under 200 words. Return only the markdown body, nothing else.",
        title
    );
    let output = tokio::process::Command::new("claude")
        .args(["--print", &prompt])
        .env_remove("CLAUDECODE")
        .output()
        .await
        .map_err(|e| format!("Failed to run claude: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Ok(String::new())
    }
}

#[tauri::command]
pub async fn create_github_issue(
    repo_path: String,
    title: String,
    body: String,
) -> Result<crate::models::GithubIssue, String> {
    // Step 1: Extract GitHub owner/repo
    let repo_path_clone = repo_path.clone();
    let nwo = tokio::task::spawn_blocking(move || extract_github_repo(&repo_path_clone))
        .await
        .map_err(|e| format!("Task failed: {}", e))??;

    // Step 2: Create the issue via gh CLI (no --json flag)
    let output = tokio::process::Command::new("gh")
        .args([
            "issue", "create",
            "--repo", &nwo,
            "--title", &title,
            "--body", &body,
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to run gh: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh issue create failed: {}", stderr));
    }

    // Step 3: Parse the issue URL from stdout
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let number = parse_github_issue_url(&url)?;

    Ok(crate::models::GithubIssue {
        number,
        title,
        url,
        labels: vec![],
    })
}

#[tauri::command]
pub async fn post_github_comment(
    repo_path: String,
    issue_number: u64,
    body: String,
) -> Result<(), String> {
    let repo_path_clone = repo_path.clone();
    let nwo = tokio::task::spawn_blocking(move || extract_github_repo(&repo_path_clone))
        .await
        .map_err(|e| format!("Task failed: {}", e))??;

    let output = tokio::process::Command::new("gh")
        .args([
            "issue", "comment",
            &issue_number.to_string(),
            "--repo", &nwo,
            "--body", &body,
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to run gh: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh issue comment failed: {}", stderr));
    }

    Ok(())
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
        let project = storage.load_project(project_uuid).map_err(|e| e.to_string())?;
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
                let _ = app_handle.emit("merge-status", format!(
                    "Rebase conflicts (attempt {}/{}). Claude is resolving...",
                    attempt + 1, MAX_MERGE_RETRIES
                ));

                // Poll until rebase is no longer in progress
                let wt_poll = worktree_path.clone();
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(REBASE_POLL_INTERVAL_SECS)).await;
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
                github_issue: None,
            },
            SessionConfig {
                id: Uuid::new_v4(),
                label: "session-2".to_string(),
                worktree_path: None,
                worktree_branch: None,
                archived: false,
                kind: "claude".to_string(),
                github_issue: None,
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
                github_issue: None,
            },
            SessionConfig {
                id: Uuid::new_v4(),
                label: "session-2".to_string(),
                worktree_path: Some("/tmp/wt2".to_string()),
                worktree_branch: Some("session-2".to_string()),
                archived: false,
                kind: "claude".to_string(),
                github_issue: None,
            },
            SessionConfig {
                id: Uuid::new_v4(),
                label: "session-3".to_string(),
                worktree_path: Some("/tmp/wt3".to_string()),
                worktree_branch: Some("session-3".to_string()),
                archived: true,
                kind: "claude".to_string(),
                github_issue: None,
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
            github_issue: None,
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

    // --- parse_github_issue_url tests ---

    #[test]
    fn test_parse_github_issue_url_basic() {
        assert_eq!(
            parse_github_issue_url("https://github.com/owner/repo/issues/42").unwrap(),
            42
        );
    }

    #[test]
    fn test_parse_github_issue_url_trailing_newline() {
        assert_eq!(
            parse_github_issue_url("https://github.com/owner/repo/issues/7\n").unwrap(),
            7
        );
    }

    #[test]
    fn test_parse_github_issue_url_invalid() {
        assert!(parse_github_issue_url("not a url").is_err());
    }
}
