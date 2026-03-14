use std::path::{Path, PathBuf};

use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::architecture::{generate_architecture_blocking, ArchitectureResult};
use crate::config;
use crate::models::{AutoWorkerQueueIssue, CommitInfo, GithubIssue, Project, SessionConfig};
use crate::state::AppState;
use crate::storage::ProjectInventory;
use crate::token_usage::{self, TokenDataPoint};
use crate::worktree::WorktreeManager;

mod github;
mod media;
mod notes;

/// Create a `CLAUDE.md` symlink pointing to `agents.md` in the given directory,
/// if `agents.md` exists and `CLAUDE.md` does not.
pub fn ensure_claude_md_symlink(dir: &Path) -> Result<(), String> {
    let claude_md = dir.join("CLAUDE.md");
    let agents_md = dir.join("agents.md");
    if agents_md.exists() && !claude_md.exists() {
        #[cfg(unix)]
        std::os::unix::fs::symlink("agents.md", &claude_md)
            .map_err(|e| format!("failed to create CLAUDE.md symlink: {}", e))?;
        #[cfg(windows)]
        std::os::windows::fs::symlink_file("agents.md", &claude_md)
            .map_err(|e| format!("failed to create CLAUDE.md symlink: {}", e))?;
    }
    Ok(())
}

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

fn update_project_with_rollback<T, C, M, R, A>(
    state: &AppState,
    project_id: Uuid,
    mutate: M,
    rollback: R,
    action: A,
) -> Result<T, String>
where
    M: FnOnce(&mut Project) -> Result<C, String>,
    R: FnOnce(&mut Project) -> Result<(), String>,
    A: FnOnce(C) -> Result<T, String>,
{
    let action_context = {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let mut project = storage
            .load_project(project_id)
            .map_err(|e| e.to_string())?;
        let action_context = mutate(&mut project)?;
        storage.save_project(&project).map_err(|e| e.to_string())?;
        action_context
    };

    match action(action_context) {
        Ok(result) => Ok(result),
        Err(action_err) => {
            let rollback = (|| -> Result<(), String> {
                let storage = state.storage.lock().map_err(|e| e.to_string())?;
                let mut project = storage
                    .load_project(project_id)
                    .map_err(|e| e.to_string())?;
                rollback(&mut project)?;
                storage.save_project(&project).map_err(|e| e.to_string())
            })();

            match rollback {
                Ok(()) => Err(action_err),
                Err(rollback_err) => Err(format!(
                    "{} (rollback failed: {})",
                    action_err, rollback_err
                )),
            }
        }
    }
}

fn cleanup_failed_session_spawn(
    repo_path: &str,
    worktree_path: Option<&str>,
    worktree_branch: Option<&str>,
) -> Result<(), String> {
    if let Some((path, branch)) = worktree_path.zip(worktree_branch) {
        WorktreeManager::remove_worktree(path, repo_path, branch)?;
    }
    Ok(())
}

async fn wait_for_merge_rebase_resolution<F, Fut>(
    mut is_rebase_in_progress: F,
    max_polls: u64,
    poll_interval: std::time::Duration,
) -> Result<(), String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<bool, String>>,
{
    for _ in 0..max_polls {
        tokio::time::sleep(poll_interval).await;
        if !is_rebase_in_progress().await? {
            return Ok(());
        }
    }

    Err("Timed out waiting for merge conflict resolution.".to_string())
}

const DEFAULT_AGENTS_MD: &str = r#"# {name}

One-line project description.

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

fn rollback_scaffold_dir(repo_path: &Path, error: String) -> String {
    match std::fs::remove_dir_all(repo_path) {
        Ok(_) => error,
        Err(cleanup_error) => format!("{} (cleanup failed: {})", error, cleanup_error),
    }
}

fn parse_github_nwo(url: &str) -> Result<String, String> {
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        return Ok(rest.trim_end_matches(".git").to_string());
    }
    if let Some(rest) = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
    {
        return Ok(rest.trim_end_matches(".git").to_string());
    }

    Err(format!("Not a GitHub remote URL: {}", url))
}

fn github_cli_command() -> std::process::Command {
    std::process::Command::new(
        std::env::var("THE_CONTROLLER_GH_BIN").unwrap_or_else(|_| "gh".to_string()),
    )
}

fn git_cli_command() -> std::process::Command {
    std::process::Command::new(
        std::env::var("THE_CONTROLLER_GIT_BIN").unwrap_or_else(|_| "git".to_string()),
    )
}

fn rollback_scaffold_state(repo_path: &Path, error: String) -> String {
    let mut cleanup_errors = Vec::new();

    if let Ok(repo) = git2::Repository::open(repo_path) {
        if let Ok(remote) = repo.find_remote("origin") {
            if let Some(url) = remote.url() {
                if let Ok(nwo) = parse_github_nwo(url) {
                    match github_cli_command()
                        .args(["repo", "delete", &nwo, "--yes"])
                        .output()
                    {
                        Ok(output) if output.status.success() => {}
                        Ok(output) => cleanup_errors.push(format!(
                            "remote cleanup failed: {}",
                            String::from_utf8_lossy(&output.stderr).trim()
                        )),
                        Err(e) => cleanup_errors.push(format!("remote cleanup failed: {}", e)),
                    }
                }
            }
        }
    }

    if let Err(cleanup_error) = std::fs::remove_dir_all(repo_path) {
        cleanup_errors.push(format!("local cleanup failed: {}", cleanup_error));
    }

    if cleanup_errors.is_empty() {
        error
    } else {
        format!("{} ({})", error, cleanup_errors.join("; "))
    }
}

fn scaffold_project_blocking(name: String, repo_path: PathBuf) -> Result<Project, String> {
    let parent_dir = repo_path
        .parent()
        .ok_or_else(|| format!("Invalid repo path: {}", repo_path.display()))?;
    std::fs::create_dir_all(parent_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir(&repo_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            format!("Directory already exists: {}", name)
        } else {
            e.to_string()
        }
    })?;
    let rollback_dir = |error: String| rollback_scaffold_dir(&repo_path, error);

    let repo = git2::Repository::init(&repo_path).map_err(|e| rollback_dir(e.to_string()))?;
    let sig = repo
        .signature()
        .unwrap_or_else(|_| git2::Signature::now("The Controller", "noreply@controller").unwrap());

    let agents_content = render_agents_md(&name);
    std::fs::write(repo_path.join("agents.md"), &agents_content)
        .map_err(|e| rollback_dir(format!("failed to write agents.md: {}", e)))?;
    ensure_claude_md_symlink(&repo_path).map_err(rollback_dir)?;
    let plans_dir = repo_path.join("docs").join("plans");
    std::fs::create_dir_all(&plans_dir)
        .map_err(|e| rollback_dir(format!("failed to create docs/plans: {}", e)))?;
    std::fs::write(plans_dir.join(".gitkeep"), "")
        .map_err(|e| rollback_dir(format!("failed to write .gitkeep: {}", e)))?;

    let mut index = repo
        .index()
        .map_err(|e| rollback_dir(format!("failed to get index: {}", e)))?;
    index
        .add_path(std::path::Path::new("agents.md"))
        .map_err(|e| rollback_dir(format!("failed to add agents.md to index: {}", e)))?;
    index
        .add_path(std::path::Path::new("CLAUDE.md"))
        .map_err(|e| rollback_dir(format!("failed to add CLAUDE.md to index: {}", e)))?;
    index
        .add_path(std::path::Path::new("docs/plans/.gitkeep"))
        .map_err(|e| rollback_dir(format!("failed to add .gitkeep to index: {}", e)))?;
    index
        .write()
        .map_err(|e| rollback_dir(format!("failed to write index: {}", e)))?;
    let tree_id = index
        .write_tree()
        .map_err(|e| rollback_dir(format!("failed to write tree: {}", e)))?;
    let tree = repo
        .find_tree(tree_id)
        .map_err(|e| rollback_dir(format!("failed to find tree: {}", e)))?;

    repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
        .map_err(|e| rollback_dir(format!("failed to create initial commit: {}", e)))?;

    let gh_output = github_cli_command()
        .args([
            "repo",
            "create",
            &name,
            "--private",
            "--source=.",
            "--remote=origin",
        ])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| rollback_dir(format!("Failed to run gh CLI: {}. Is gh installed?", e)))?;
    if !gh_output.status.success() {
        let stderr = String::from_utf8_lossy(&gh_output.stderr);
        return Err(rollback_dir(format!(
            "Failed to create GitHub repo: {}",
            stderr.trim()
        )));
    }

    let push_output = git_cli_command()
        .args(["push", "--set-upstream", "origin", "HEAD"])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| {
            rollback_scaffold_state(&repo_path, format!("Failed to run git push: {}", e))
        })?;
    if !push_output.status.success() {
        let stderr = String::from_utf8_lossy(&push_output.stderr);
        return Err(rollback_scaffold_state(
            &repo_path,
            format!("Failed to push initial commit: {}", stderr.trim()),
        ));
    }

    Ok(Project {
        id: Uuid::new_v4(),
        name,
        repo_path: repo_path.to_string_lossy().to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        archived: false,
        maintainer: crate::models::MaintainerConfig::default(),
        auto_worker: crate::models::AutoWorkerConfig::default(),
        prompts: vec![],
        sessions: vec![],
        staged_session: None,
    })
}

/// Run storage migrations on startup (worktree path format, etc.).
/// PTY connections are deferred to `connect_session` so each terminal
/// can attach at the correct size.
#[tauri::command]
pub fn restore_sessions(state: State<AppState>) -> Result<(), String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let inventory = storage.list_projects().map_err(|e| e.to_string())?;
    inventory.warn_if_corrupt("restore_sessions");
    // Migrate worktree paths from UUID-based to name-based directories
    for project in &inventory.projects {
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
///
/// This command is async because it shells out to tmux (create, resize, attach),
/// which would block the main thread and prevent event delivery — including the
/// alternate-screen escape sequence that xterm.js needs for correct scrolling.
#[tauri::command]
pub async fn connect_session(
    state: State<'_, AppState>,
    _app_handle: AppHandle,
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
        let inventory = storage.list_projects().map_err(|e| e.to_string())?;
        inventory.warn_if_corrupt("connect_session");
        inventory
            .projects
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

    // Run on a background thread to avoid blocking the main thread.
    // This is critical: the reader thread spawned inside spawn_session emits
    // pty-output events immediately, and the main thread must be free to
    // deliver them to the webview (especially the smcup/alternate-screen escape).
    let pty_manager = state.pty_manager.clone();
    let emitter = state.emitter.clone();
    tokio::task::spawn_blocking(move || {
        let mut mgr = pty_manager.lock().map_err(|e| e.to_string())?;
        mgr.spawn_session(id, &session_dir, &kind, emitter, true, None, rows, cols)
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}

#[tauri::command]
pub fn create_project(
    state: State<AppState>,
    name: String,
    repo_path: String,
) -> Result<Project, String> {
    validate_project_name(&name)?;

    let path = Path::new(&repo_path);
    if !path.is_dir() {
        return Err(format!("repo_path is not a directory: {}", repo_path));
    }

    let storage = state.storage.lock().map_err(|e| e.to_string())?;

    // Reject duplicate project names.
    if let Ok(inventory) = storage.list_projects() {
        let existing = inventory.projects;
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
        maintainer: crate::models::MaintainerConfig::default(),
        auto_worker: crate::models::AutoWorkerConfig::default(),
        prompts: vec![],
        sessions: vec![],
        staged_session: None,
    };

    storage.save_project(&project).map_err(|e| e.to_string())?;

    // If repo doesn't have agents.md, create default one in config dir
    let repo_agents = path.join("agents.md");
    if !repo_agents.exists() {
        storage
            .save_agents_md(project.id, &render_agents_md(&project.name))
            .map_err(|e| e.to_string())?;
    }

    // If repo has agents.md but no CLAUDE.md, create symlink
    ensure_claude_md_symlink(path)?;

    Ok(project)
}

#[tauri::command]
pub fn load_project(
    state: State<AppState>,
    name: String,
    repo_path: String,
) -> Result<Project, String> {
    validate_project_name(&name)?;

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
    if let Ok(inventory) = storage.list_projects() {
        let existing = inventory.projects;
        if let Some(project) = existing.iter().find(|p| p.repo_path == repo_path) {
            return Ok(project.clone());
        }
        // Reject duplicate project names when creating new.
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
        maintainer: crate::models::MaintainerConfig::default(),
        auto_worker: crate::models::AutoWorkerConfig::default(),
        prompts: vec![],
        sessions: vec![],
        staged_session: None,
    };

    storage.save_project(&project).map_err(|e| e.to_string())?;

    // Only create default agents.md if repo doesn't have one
    let repo_agents = path.join("agents.md");
    if !repo_agents.exists() {
        storage
            .save_agents_md(project.id, &render_agents_md(&project.name))
            .map_err(|e| e.to_string())?;
    }

    // If repo has agents.md but no CLAUDE.md, create symlink
    ensure_claude_md_symlink(path)?;

    Ok(project)
}

#[tauri::command]
pub fn list_projects(state: State<AppState>) -> Result<ProjectInventory, String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let inventory = storage.list_projects().map_err(|e| e.to_string())?;
    Ok(inventory)
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
    if delete_repo && Path::new(&project.repo_path).exists() {
        std::fs::remove_dir_all(&project.repo_path)
            .map_err(|e| format!("failed to delete repo: {}", e))?;
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
    _app_handle: AppHandle,
    project_id: String,
    kind: Option<String>,
    github_issue: Option<crate::models::GithubIssue>,
    background: Option<bool>,
    initial_prompt: Option<String>,
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

    // Build initial prompt: explicit prompt takes priority, then GitHub issue context
    let initial_prompt = initial_prompt.or_else(|| {
        github_issue.as_ref().map(|issue| {
            crate::session_args::build_issue_prompt(
                issue.number,
                &issue.title,
                &issue.url,
                background,
            )
        })
    });
    let rollback_worktree = wt_path.clone().zip(wt_branch.clone());

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
        auto_worker_session: false,
    };

    update_project_with_rollback(
        &state,
        project_uuid,
        |project| {
            project.sessions.push(session_config);
            Ok(())
        },
        |project| {
            project.sessions.retain(|session| session.id != session_id);
            Ok(())
        },
        |()| {
            let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
            pty_manager.spawn_session(
                session_id,
                &session_dir,
                &kind,
                state.emitter.clone(),
                false,
                initial_prompt.as_deref(),
                24,
                80,
            )
        },
    )
    .map_err(|spawn_err| {
        if let Some((ref worktree_path, ref worktree_branch)) = rollback_worktree {
            if let Err(cleanup_err) = cleanup_failed_session_spawn(
                &repo_path,
                Some(worktree_path.as_str()),
                Some(worktree_branch.as_str()),
            ) {
                return format!("{} (worktree cleanup failed: {})", spawn_err, cleanup_err);
            }
        }
        spawn_err
    })?;

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
pub async fn submit_secure_env_value(
    state: State<'_, AppState>,
    request_id: String,
    value: String,
) -> Result<String, String> {
    let (pending, response_tx) =
        crate::secure_env::take_secure_env_submission(&state, &request_id)?;
    let request_id_for_blocking = request_id.clone();
    let value_for_blocking = value;
    let result = tokio::task::spawn_blocking(move || {
        crate::secure_env::update_env_file(&pending.env_path, &pending.key, &value_for_blocking)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?;
    let result = crate::secure_env::finish_secure_env_submission(
        &request_id_for_blocking,
        response_tx,
        result,
    )?;
    Ok(if result.created {
        "created".to_string()
    } else {
        "updated".to_string()
    })
}

#[tauri::command]
pub fn cancel_secure_env_request(
    state: State<AppState>,
    request_id: String,
) -> Result<(), String> {
    crate::secure_env::cancel_secure_env_request(&state, &request_id)
}

const COMMIT_POLL_INTERVAL_SECS: u64 = 3;
const MAX_COMMIT_WAIT_SECS: u64 = 60;
const MAX_REBASE_WAIT_SECS: u64 = 360; // 6 minutes
const STAGING_PORT_OFFSET: u16 = 1000;

/// Find a free port for the staged Controller instance.
/// Starts at base_port + 1000 and increments until a free port is found.
fn find_staging_port(base_port: u16) -> Result<u16, String> {
    let start = base_port
        .checked_add(STAGING_PORT_OFFSET)
        .ok_or("Port overflow")?;
    for candidate in start..start.saturating_add(100) {
        let ipv4_free = std::net::TcpListener::bind(("127.0.0.1", candidate)).is_ok();
        let ipv6_free = std::net::TcpListener::bind(("::1", candidate)).is_ok();
        if ipv4_free && ipv6_free {
            return Ok(candidate);
        }
    }
    Err(format!(
        "No free port found in range {}-{}",
        start,
        start.saturating_add(99)
    ))
}

/// Kill a process group by PID. Sends SIGTERM to the group, then SIGKILL after 2s
/// if the group is still alive.
pub(crate) fn kill_process_group(pid: u32) {
    #[cfg(unix)]
    {
        use libc::{kill, SIGTERM, SIGKILL};
        if let Ok(pgid) = i32::try_from(pid) {
            unsafe { kill(-pgid, SIGTERM); }
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(2));
                // Only send SIGKILL if the process group is still alive
                // (kill with signal 0 checks existence without sending a signal)
                if unsafe { kill(-pgid, 0) } == 0 {
                    unsafe { kill(-pgid, SIGKILL); }
                }
            });
        }
    }
}

#[tauri::command]
pub async fn stage_session(
    state: State<'_, AppState>,
    _app_handle: AppHandle,
    project_id: String,
    session_id: String,
) -> Result<(), String> {
    use crate::models::StagedSession;
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;

    let project_uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let session_uuid = Uuid::parse_str(&session_id).map_err(|e| e.to_string())?;

    let _staging_guard = state.staging_lock.lock().await;

    // Extract data under a short-lived storage lock to avoid deadlock with pty_manager
    let (repo_path, branch, worktree_path) = {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let project = storage
            .load_project(project_uuid)
            .map_err(|e| e.to_string())?;

        if project.name != "the-controller" {
            return Err("Staging is only supported for the-controller".to_string());
        }

        if let Some(staged) = &project.staged_session {
            // Check if the staged process is still alive
            #[cfg(unix)]
            let alive = unsafe { libc::kill(staged.pid as i32, 0) } == 0;
            #[cfg(not(unix))]
            let alive = false;
            if alive {
                return Err("A session is already staged — unstage it first".to_string());
            }
            // Stale record — kill orphaned children (e.g. Vite, esbuild that outlived
            // the process leader), clean up the socket, then clear the record.
            kill_process_group(staged.pid);
            let _ = std::fs::remove_file(crate::status_socket::staged_socket_path());
            let mut p = project.clone();
            p.staged_session = None;
            storage.save_project(&p).map_err(|e| e.to_string())?;
        }

        let session = project
            .sessions
            .iter()
            .find(|s| s.id == session_uuid)
            .ok_or("Session not found")?;

        let branch = session
            .worktree_branch
            .as_deref()
            .ok_or("Session has no worktree branch")?
            .to_string();

        let worktree_path = session
            .worktree_path
            .as_deref()
            .ok_or("Session has no worktree path")?
            .to_string();

        (project.repo_path.clone(), branch, worktree_path)
    };

    // 1. Ensure worktree is clean — prompt Claude to commit if needed
    {
        let wt = worktree_path.clone();
        let is_clean = tokio::task::spawn_blocking(move || WorktreeManager::is_worktree_clean(&wt))
            .await
            .map_err(|e| format!("Task failed: {}", e))??;

        if !is_clean {
            let prompt = "\nYou have uncommitted changes. Please commit all your work now.\r";
            {
                let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
                let _ = pty_manager.write_to_session(session_uuid, prompt.as_bytes());
            }

            let _ = state
                .emitter
                .emit("staging-status", "Waiting for commit...");

            let max_polls = MAX_COMMIT_WAIT_SECS / COMMIT_POLL_INTERVAL_SECS;
            let mut committed = false;
            for _ in 0..max_polls {
                tokio::time::sleep(std::time::Duration::from_secs(COMMIT_POLL_INTERVAL_SECS)).await;
                let wt_check = worktree_path.clone();
                let clean = tokio::task::spawn_blocking(move || {
                    WorktreeManager::is_worktree_clean(&wt_check)
                })
                .await
                .map_err(|e| format!("Task failed: {}", e))??;
                if clean {
                    committed = true;
                    break;
                }
            }
            if !committed {
                return Err(
                    "Timed out waiting for commit. Please commit manually and retry.".to_string(),
                );
            }
        }
    }

    // 2. Rebase onto main if needed
    {
        let rp = repo_path.clone();
        let main_branch =
            tokio::task::spawn_blocking(move || WorktreeManager::detect_main_branch(&rp))
                .await
                .map_err(|e| format!("Task failed: {}", e))??;

        let rp = repo_path.clone();
        let _ = tokio::task::spawn_blocking(move || WorktreeManager::sync_main(&rp)).await;

        let rp = repo_path.clone();
        let br = branch.clone();
        let mb = main_branch.clone();
        let is_behind =
            tokio::task::spawn_blocking(move || WorktreeManager::is_branch_behind(&rp, &br, &mb))
                .await
                .map_err(|e| format!("Task failed: {}", e))??;

        if is_behind {
            let wt = worktree_path.clone();
            let mb = main_branch.clone();
            let rebase_clean =
                tokio::task::spawn_blocking(move || WorktreeManager::rebase_onto(&wt, &mb))
                    .await
                    .map_err(|e| format!("Task failed: {}", e))??;

            if !rebase_clean {
                // Rebase has conflicts — ask Claude to resolve
                let prompt = "\nThere are rebase conflicts. Please resolve all conflicts, then run `git rebase --continue`.\r";
                {
                    let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
                    let _ = pty_manager.write_to_session(session_uuid, prompt.as_bytes());
                }

                let _ = state
                    .emitter
                    .emit("staging-status", "Rebase conflicts. Claude is resolving...");

                // Poll until rebase is no longer in progress
                let max_polls = MAX_REBASE_WAIT_SECS / REBASE_POLL_INTERVAL_SECS;
                let mut resolved = false;
                for _ in 0..max_polls {
                    tokio::time::sleep(std::time::Duration::from_secs(REBASE_POLL_INTERVAL_SECS))
                        .await;
                    let wt_check = worktree_path.clone();
                    let still_rebasing = tokio::task::spawn_blocking(move || {
                        WorktreeManager::is_rebase_in_progress(&wt_check)
                    })
                    .await
                    .map_err(|e| format!("Task failed: {}", e))?;
                    if !still_rebasing {
                        resolved = true;
                        break;
                    }
                }
                if !resolved {
                    return Err("Timed out waiting for rebase conflict resolution.".to_string());
                }
            }
        }
    }

    // 3. Launch a separate Controller instance from the worktree
    let _ = state
        .emitter
        .emit("staging-status", "Preparing staged instance...");

    // Ensure node_modules exists in the worktree
    let node_modules = PathBuf::from(&worktree_path).join("node_modules");
    if !node_modules.exists() {
        let _ = state
            .emitter
            .emit("staging-status", "Installing dependencies...");
        let wt = worktree_path.clone();
        let install_status = tokio::task::spawn_blocking(move || {
            std::process::Command::new("npm")
                .arg("install")
                .current_dir(&wt)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
        })
        .await
        .map_err(|e| format!("Task failed: {}", e))?
        .map_err(|e| format!("npm install failed: {}", e))?;

        if !install_status.success() {
            return Err("npm install failed in worktree".to_string());
        }
    }

    let port = find_staging_port(1420)?;

    let _ = state
        .emitter
        .emit("staging-status", &format!("Starting on port {}...", port));

    let wt = worktree_path.clone();
    let log_path = PathBuf::from(&wt).join("staging.log");
    let log_file = std::fs::File::create(&log_path)
        .map_err(|e| format!("Failed to create staging log: {}", e))?;
    let log_stderr = log_file
        .try_clone()
        .map_err(|e| format!("Failed to clone log file: {}", e))?;
    let mut child = std::process::Command::new("bash")
        .args(["./dev.sh", &port.to_string()])
        .current_dir(&wt)
        .env("CONTROLLER_SOCKET", crate::status_socket::staged_socket_path())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(log_stderr))
        .process_group(0)
        .spawn()
        .map_err(|e| format!("Failed to spawn staged instance: {}", e))?;

    let pid = child.id();
    // Reap the child in a background thread to prevent zombie entries.
    // We manage the process lifetime via PID/process group (kill_process_group),
    // not via this Child handle.
    std::thread::spawn(move || {
        let _ = child.wait();
    });

    // Save staged session info — if save fails, kill the orphan process
    let save_result = (|| -> Result<(), String> {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let mut project = storage
            .load_project(project_uuid)
            .map_err(|e| e.to_string())?;

        project.staged_session = Some(StagedSession {
            session_id: session_uuid,
            pid,
            port,
        });

        storage.save_project(&project).map_err(|e| e.to_string())
    })();

    if let Err(e) = save_result {
        kill_process_group(pid);
        return Err(e);
    }

    Ok(())
}

#[tauri::command]
pub fn unstage_session(state: State<AppState>, project_id: String) -> Result<(), String> {
    let project_uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage
        .load_project(project_uuid)
        .map_err(|e| e.to_string())?;

    let staged = project
        .staged_session
        .take()
        .ok_or("No session is currently staged")?;

    // Kill the staged Controller process group
    kill_process_group(staged.pid);

    // Clean up the staged socket
    let _ = std::fs::remove_file(crate::status_socket::staged_socket_path());

    storage.save_project(&project).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_repo_head(repo_path: String) -> Result<(String, String), String> {
    let repo =
        git2::Repository::open(&repo_path).map_err(|e| format!("Failed to open repo: {}", e))?;

    let head = repo
        .head()
        .map_err(|e| format!("Failed to get HEAD: {}", e))?;
    let branch = head.shorthand().unwrap_or("HEAD").to_string();

    let commit = head
        .peel_to_commit()
        .map_err(|e| format!("Failed to peel to commit: {}", e))?;
    let short_hash = commit.id().to_string()[..7].to_string();

    Ok((branch, short_hash))
}

#[tauri::command]
pub fn save_session_prompt(
    state: State<AppState>,
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
        .iter()
        .find(|s| s.id == session_uuid)
        .ok_or_else(|| "Session not found".to_string())?;

    // Build prompt text: use initial_prompt, or derive from github_issue
    let prompt_text = session
        .initial_prompt
        .clone()
        .or_else(|| {
            session.github_issue.as_ref().map(|issue| {
                crate::session_args::build_issue_prompt(
                    issue.number,
                    &issue.title,
                    &issue.url,
                    false,
                )
            })
        })
        .ok_or_else(|| "Session has no prompt to save".to_string())?;

    // Auto-generate name: first ~60 chars (safe for multi-byte UTF-8)
    let name = {
        let truncated: String = prompt_text.chars().take(60).collect();
        if truncated.len() < prompt_text.len() {
            format!("{}...", truncated)
        } else {
            truncated
        }
    };

    let saved = crate::models::SavedPrompt {
        id: Uuid::new_v4(),
        name,
        text: prompt_text,
        created_at: chrono::Utc::now().to_rfc3339(),
        source_session_label: session.label.clone(),
    };

    project.prompts.push(saved);
    storage.save_project(&project).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn list_project_prompts(
    state: State<AppState>,
    project_id: String,
) -> Result<Vec<crate::models::SavedPrompt>, String> {
    let project_uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let project = storage
        .load_project(project_uuid)
        .map_err(|e| e.to_string())?;
    Ok(project.prompts)
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

    // Try to close the PTY session even if the terminal is already gone.
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
pub fn start_claude_login(
    state: State<AppState>,
    _app_handle: AppHandle,
) -> Result<String, String> {
    let session_id = Uuid::new_v4();
    let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
    pty_manager.spawn_command(session_id, "claude", &["login"], state.emitter.clone())?;
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
    let result = tokio::task::spawn_blocking(config::check_claude_cli_status)
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
pub async fn generate_architecture(repo_path: String) -> Result<ArchitectureResult, String> {
    tokio::task::spawn_blocking(move || {
        generate_architecture_blocking(std::path::Path::new(&repo_path))
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}

#[tauri::command]
pub async fn scaffold_project(state: State<'_, AppState>, name: String) -> Result<Project, String> {
    validate_project_name(&name)?;

    let repo_path = {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;

        // Reject duplicate project names.
        if let Ok(inventory) = storage.list_projects() {
            let existing = inventory.projects;
            if existing.iter().any(|p| p.name == name) {
                return Err(format!("A project named '{}' already exists", name));
            }
        }

        let cfg = config::load_config(&storage.base_dir())
            .ok_or_else(|| "No config found. Complete onboarding first.".to_string())?;

        std::path::Path::new(&cfg.projects_root).join(&name)
    };
    if repo_path.exists() {
        return Err(format!("Directory already exists: {}", name));
    }

    let project = tokio::task::spawn_blocking(move || scaffold_project_blocking(name, repo_path))
        .await
        .map_err(|e| format!("Task failed: {}", e))??;

    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    if let Ok(inventory) = storage.list_projects() {
        let existing = inventory.projects;
        if existing.iter().any(|p| p.name == project.name) {
            drop(storage);
            return Err(rollback_scaffold_state(
                Path::new(&project.repo_path),
                format!("A project named '{}' already exists", project.name),
            ));
        }
    }
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
pub async fn list_assigned_issues(
    repo_path: String,
) -> Result<Vec<crate::models::AssignedIssue>, String> {
    github::list_assigned_issues(repo_path).await
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
    description: Option<String>,
    color: Option<String>,
) -> Result<(), String> {
    github::add_github_label(state, repo_path, issue_number, label, description, color).await
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
pub async fn capture_app_screenshot(app: AppHandle, cropped: bool) -> Result<String, String> {
    media::capture_app_screenshot(app, cropped).await
}

#[tauri::command]
pub fn list_notes(
    state: State<'_, AppState>,
    folder: String,
) -> Result<Vec<crate::notes::NoteEntry>, String> {
    notes::list_notes(state, folder)
}

#[tauri::command]
pub fn read_note(
    state: State<'_, AppState>,
    folder: String,
    filename: String,
) -> Result<String, String> {
    notes::read_note(state, folder, filename)
}

#[tauri::command]
pub fn write_note(
    state: State<'_, AppState>,
    folder: String,
    filename: String,
    content: String,
) -> Result<(), String> {
    notes::write_note(state, folder, filename, content)
}

#[tauri::command]
pub fn create_note(
    state: State<'_, AppState>,
    folder: String,
    title: String,
) -> Result<String, String> {
    notes::create_note(state, folder, title)
}

#[tauri::command]
pub fn rename_note(
    state: State<'_, AppState>,
    folder: String,
    old_name: String,
    new_name: String,
) -> Result<String, String> {
    notes::rename_note(state, folder, old_name, new_name)
}

#[tauri::command]
pub fn duplicate_note(
    state: State<'_, AppState>,
    folder: String,
    filename: String,
) -> Result<String, String> {
    notes::duplicate_note(state, folder, filename)
}

#[tauri::command]
pub fn delete_note(
    state: State<'_, AppState>,
    folder: String,
    filename: String,
) -> Result<(), String> {
    notes::delete_note(state, folder, filename)
}

#[tauri::command]
pub fn list_folders(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    notes::list_folders(state)
}

#[tauri::command]
pub fn create_folder(state: State<'_, AppState>, name: String) -> Result<(), String> {
    notes::create_folder(state, name)
}

#[tauri::command]
pub fn rename_folder(state: State<'_, AppState>, old_name: String, new_name: String) -> Result<(), String> {
    notes::rename_folder(state, old_name, new_name)
}

#[tauri::command]
pub fn delete_folder(state: State<'_, AppState>, name: String, force: bool) -> Result<(), String> {
    notes::delete_folder(state, name, force)
}

#[tauri::command]
pub fn commit_notes(state: State<'_, AppState>) -> Result<bool, String> {
    notes::commit_notes(state)
}

#[tauri::command]
pub fn save_note_image(
    state: State<'_, AppState>,
    folder: String,
    image_bytes: Vec<u8>,
    extension: String,
) -> Result<String, String> {
    let base_dir = state
        .storage
        .lock()
        .map_err(|e| e.to_string())?
        .base_dir();
    crate::notes::save_note_image(&base_dir, &folder, &image_bytes, &extension)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn resolve_note_asset_path(
    state: State<'_, AppState>,
    folder: String,
    relative_path: String,
) -> Result<String, String> {
    let base_dir = state
        .storage
        .lock()
        .map_err(|e| e.to_string())?
        .base_dir();
    crate::notes::resolve_note_asset_path(&base_dir, &folder, &relative_path)
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn send_note_ai_chat(
    note_content: String,
    selected_text: String,
    conversation_history: Vec<crate::note_ai_chat::NoteAiChatMessage>,
    prompt: String,
) -> Result<crate::note_ai_chat::NoteAiResponse, String> {
    crate::note_ai_chat::send_note_ai_message(
        std::env::temp_dir().to_string_lossy().to_string(),
        note_content,
        selected_text,
        conversation_history,
        prompt,
    )
    .await
}
const MAX_MERGE_RETRIES: u32 = 5;
const REBASE_POLL_INTERVAL_SECS: u64 = 3;
const MAX_MERGE_REBASE_WAIT_SECS: u64 = 600; // 10 minutes

#[tauri::command]
pub async fn merge_session_branch(
    state: State<'_, AppState>,
    _app_handle: AppHandle,
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
                let _ = state.emitter.emit(
                    "merge-status",
                    &format!(
                        "Rebase conflicts (attempt {}/{}). Claude is resolving...",
                        attempt + 1,
                        MAX_MERGE_RETRIES
                    ),
                );

                // Poll until rebase is no longer in progress, but stop waiting
                // eventually so the frontend can recover if Claude never resolves it.
                let wt_poll = worktree_path.clone();
                wait_for_merge_rebase_resolution(
                    move || {
                        let wt_check = wt_poll.clone();
                        async move {
                            tokio::task::spawn_blocking(move || {
                                WorktreeManager::is_rebase_in_progress(&wt_check)
                            })
                            .await
                            .map_err(|e| format!("Task failed: {}", e))
                        }
                    },
                    MAX_MERGE_REBASE_WAIT_SECS / REBASE_POLL_INTERVAL_SECS,
                    std::time::Duration::from_secs(REBASE_POLL_INTERVAL_SECS),
                )
                .await?;

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

#[tauri::command]
pub fn get_session_token_usage(
    state: State<AppState>,
    project_id: String,
    session_id: String,
) -> Result<Vec<TokenDataPoint>, String> {
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

    let working_dir = session
        .worktree_path
        .as_deref()
        .unwrap_or(&project.repo_path);

    token_usage::get_token_usage(working_dir, &session.kind)
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
        let hash = oid.to_string()[..7].to_string();
        commits.push(CommitInfo { hash, message });
        if commits.len() >= 20 {
            break;
        }
    }

    Ok(commits)
}

pub(crate) fn validate_maintainer_interval(minutes: u64) -> Result<(), String> {
    if minutes < 5 {
        return Err("Interval must be at least 5 minutes".to_string());
    }
    Ok(())
}

#[tauri::command]
pub async fn configure_maintainer(
    state: State<'_, AppState>,
    project_id: String,
    enabled: bool,
    interval_minutes: u64,
    github_repo: Option<String>,
) -> Result<(), String> {
    validate_maintainer_interval(interval_minutes)?;
    let project_id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage
        .load_project(project_id)
        .map_err(|e| e.to_string())?;
    project.maintainer.enabled = enabled;
    project.maintainer.interval_minutes = interval_minutes;
    project.maintainer.github_repo = github_repo;
    storage.save_project(&project).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn configure_auto_worker(
    state: State<'_, AppState>,
    project_id: String,
    enabled: bool,
) -> Result<(), String> {
    let project_id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage
        .load_project(project_id)
        .map_err(|e| e.to_string())?;
    project.auto_worker.enabled = enabled;
    storage.save_project(&project).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_worker_reports(repo_path: String) -> Result<Vec<github::WorkerReport>, String> {
    github::get_worker_reports(repo_path).await
}

fn queue_issue_from_github(issue: GithubIssue, is_active: bool) -> AutoWorkerQueueIssue {
    AutoWorkerQueueIssue {
        number: issue.number,
        title: issue.title,
        url: issue.url,
        body: issue.body,
        labels: issue.labels.into_iter().map(|label| label.name).collect(),
        is_active,
    }
}

fn active_auto_worker_issue(project: &Project) -> Option<GithubIssue> {
    project
        .sessions
        .iter()
        .find(|session| session.auto_worker_session)
        .and_then(|session| session.github_issue.clone())
}

fn build_auto_worker_queue(
    issues: Vec<GithubIssue>,
    active_issue: Option<GithubIssue>,
) -> Vec<AutoWorkerQueueIssue> {
    let active_issue_number = active_issue.as_ref().map(|issue| issue.number);
    let mut queue = Vec::new();

    if let Some(issue) = active_issue {
        queue.push(queue_issue_from_github(issue, true));
    }

    queue.extend(
        issues
            .into_iter()
            .filter(crate::auto_worker::is_eligible)
            .filter(|issue| Some(issue.number) != active_issue_number)
            .map(|issue| queue_issue_from_github(issue, false)),
    );

    queue
}

#[tauri::command]
pub async fn get_auto_worker_queue(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<AutoWorkerQueueIssue>, String> {
    let project_id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let project = {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        storage
            .load_project(project_id)
            .map_err(|e| e.to_string())?
    };

    let active_issue = active_auto_worker_issue(&project);
    let issues = github::list_github_issues(project.repo_path.clone(), state).await?;
    Ok(build_auto_worker_queue(issues, active_issue))
}

#[tauri::command]
pub async fn get_maintainer_status(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Option<crate::models::MaintainerRunLog>, String> {
    let project_id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    storage
        .latest_maintainer_run_log(project_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_maintainer_history(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<crate::models::MaintainerRunLog>, String> {
    let project_id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    storage
        .maintainer_run_log_history(project_id, 20)
        .map_err(|e| e.to_string())
}

async fn run_maintainer_check_spawn_blocking_with<F>(
    repo_path: String,
    project_id: Uuid,
    github_repo: Option<String>,
    runner: F,
) -> Result<crate::models::MaintainerRunLog, String>
where
    F: FnOnce(String, Uuid, Option<String>) -> Result<crate::models::MaintainerRunLog, String>
        + Send
        + 'static,
{
    tokio::task::spawn_blocking(move || runner(repo_path, project_id, github_repo))
        .await
        .map_err(|e| format!("Task failed: {e}"))?
}

async fn run_maintainer_check_spawn_blocking(
    repo_path: String,
    project_id: Uuid,
    github_repo: Option<String>,
) -> Result<crate::models::MaintainerRunLog, String> {
    run_maintainer_check_spawn_blocking_with(
        repo_path,
        project_id,
        github_repo,
        |repo_path, project_id, github_repo| {
            crate::maintainer::run_maintainer_check(&repo_path, project_id, github_repo.as_deref())
        },
    )
    .await
}

#[tauri::command]
pub async fn trigger_maintainer_check(
    state: State<'_, AppState>,
    _app_handle: AppHandle,
    project_id: String,
) -> Result<crate::models::MaintainerRunLog, String> {
    let project_id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let (repo_path, github_repo) = {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let project = storage
            .load_project(project_id)
            .map_err(|e| e.to_string())?;
        (
            project.repo_path.clone(),
            project.maintainer.github_repo.clone(),
        )
    };

    let _ = state
        .emitter
        .emit(&format!("maintainer-status:{}", project_id), "running");

    let log = match run_maintainer_check_spawn_blocking(repo_path, project_id, github_repo).await {
        Ok(log) => log,
        Err(e) => {
            let _ = state
                .emitter
                .emit(&format!("maintainer-status:{}", project_id), "error");
            let _ = state
                .emitter
                .emit(&format!("maintainer-error:{}", project_id), &e.to_string());
            return Err(e);
        }
    };

    {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        storage
            .save_maintainer_run_log(&log)
            .map_err(|e| e.to_string())?;
    }

    let _ = state
        .emitter
        .emit(&format!("maintainer-status:{}", project_id), "idle");

    Ok(log)
}

#[tauri::command]
pub async fn clear_maintainer_reports(
    state: State<'_, AppState>,
    _app_handle: AppHandle,
    project_id: String,
) -> Result<(), String> {
    let project_id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    storage
        .clear_maintainer_run_logs(project_id)
        .map_err(|e| e.to_string())?;
    let _ = state
        .emitter
        .emit(&format!("maintainer-status:{}", project_id), "idle");
    Ok(())
}

#[tauri::command]
pub async fn get_maintainer_issues(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<crate::models::MaintainerIssue>, String> {
    let project_id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let (repo_path, github_repo) = {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let project = storage
            .load_project(project_id)
            .map_err(|e| e.to_string())?;
        (
            project.repo_path.clone(),
            project.maintainer.github_repo.clone(),
        )
    };
    github::get_maintainer_issues(repo_path, github_repo).await
}

#[tauri::command]
pub async fn get_maintainer_issue_detail(
    state: State<'_, AppState>,
    project_id: String,
    issue_number: u32,
) -> Result<crate::models::MaintainerIssueDetail, String> {
    let project_id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let (repo_path, github_repo) = {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let project = storage
            .load_project(project_id)
            .map_err(|e| e.to_string())?;
        (
            project.repo_path.clone(),
            project.maintainer.github_repo.clone(),
        )
    };
    github::get_maintainer_issue_detail(repo_path, github_repo, issue_number).await
}

#[tauri::command]
pub fn log_frontend_error(message: String) {
    eprintln!("[FRONTEND] {}", message);
}

#[tauri::command]
pub async fn start_voice_pipeline(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut pipeline = state.voice_pipeline.lock().await;
    if pipeline.is_some() {
        return Ok(()); // Already running
    }
    let emitter = state.emitter.clone();
    let new_pipeline = crate::voice::VoicePipeline::start(emitter).await?;
    *pipeline = Some(new_pipeline);
    Ok(())
}

#[tauri::command]
pub async fn stop_voice_pipeline(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut pipeline = state.voice_pipeline.lock().await;
    if let Some(mut p) = pipeline.take() {
        p.stop();
    }
    Ok(())
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
    use crate::config::{save_config, Config};
    use crate::models::{MaintainerRunLog, SavedPrompt, SessionConfig};
    use crate::pty_manager::PtyManager;
    use crate::state::{AppState, IssueCache};
    use crate::storage::Storage;
    use once_cell::sync::Lazy;
    use std::env;
    use std::fs;
    use std::future::Future;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;
    use tempfile::TempDir;
    use uuid::Uuid;

    static ENV_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    fn make_test_state(base_dir: &Path, projects_root: &Path) -> AppState {
        let storage = Storage::new(base_dir.to_path_buf());
        storage.ensure_dirs().expect("ensure_dirs");
        save_config(
            base_dir,
            &Config {
                projects_root: projects_root.to_string_lossy().to_string(),
            },
        )
        .expect("save_config");

        AppState {
            storage: Mutex::new(storage),
            pty_manager: Arc::new(Mutex::new(PtyManager::new())),
            issue_cache: Arc::new(Mutex::new(IssueCache::new())),
            secure_env_request: Mutex::new(None),
            emitter: crate::emitter::NoopEmitter::new(),
            staging_lock: tokio::sync::Mutex::new(()),
            voice_pipeline: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    fn state_from_ref<T: Send + Sync + 'static>(value: &T) -> tauri::State<'_, T> {
        unsafe { std::mem::transmute(value) }
    }

    fn run_async_test<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build test runtime")
            .block_on(future)
    }

    fn real_git_path() -> String {
        let output = std::process::Command::new("which")
            .arg("git")
            .output()
            .expect("locate git");
        assert!(output.status.success(), "which git should succeed");
        String::from_utf8(output.stdout)
            .expect("utf8 git path")
            .trim()
            .to_string()
    }

    fn write_fake_command(path: &Path, body: &str) {
        fs::write(path, format!("#!/bin/sh\n{}\n", body)).expect("write fake command");
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(path).expect("stat fake command").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(path, perms).expect("chmod fake command");
        }
    }

    fn with_fake_cli_bins<T>(f: impl FnOnce(&Path, &Path, &Path) -> T) -> T {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let bin_dir = TempDir::new().expect("temp fake cli dir");
        let state_dir = TempDir::new().expect("temp scaffold state dir");
        let gh_path = bin_dir.path().join("gh");
        let git_path = bin_dir.path().join("git");
        let original_gh = env::var_os("THE_CONTROLLER_GH_BIN");
        let original_git = env::var_os("THE_CONTROLLER_GIT_BIN");

        env::set_var("THE_CONTROLLER_GH_BIN", &gh_path);
        env::set_var("THE_CONTROLLER_GIT_BIN", &git_path);

        let result = f(&gh_path, &git_path, state_dir.path());

        match original_gh {
            Some(path) => env::set_var("THE_CONTROLLER_GH_BIN", path),
            None => env::remove_var("THE_CONTROLLER_GH_BIN"),
        }
        match original_git {
            Some(path) => env::set_var("THE_CONTROLLER_GIT_BIN", path),
            None => env::remove_var("THE_CONTROLLER_GIT_BIN"),
        }

        result
    }

    #[test]
    fn test_wait_for_merge_rebase_resolution_times_out() {
        run_async_test(async {
            let checks = Arc::new(AtomicUsize::new(0));
            let checks_for_closure = Arc::clone(&checks);

            let result = wait_for_merge_rebase_resolution(
                move || {
                    let checks_for_closure = Arc::clone(&checks_for_closure);
                    async move {
                        checks_for_closure.fetch_add(1, Ordering::SeqCst);
                        Ok(true)
                    }
                },
                2,
                Duration::from_millis(1),
            )
            .await;
            assert_eq!(
                result.expect_err("wait should time out"),
                "Timed out waiting for merge conflict resolution."
            );
            assert_eq!(checks.load(Ordering::SeqCst), 2);
        });
    }

    #[test]
    fn test_wait_for_merge_rebase_resolution_stops_once_rebase_clears() {
        run_async_test(async {
            let checks = Arc::new(AtomicUsize::new(0));
            let checks_for_closure = Arc::clone(&checks);

            let result = wait_for_merge_rebase_resolution(
                move || {
                    let checks_for_closure = Arc::clone(&checks_for_closure);
                    async move {
                        let attempt = checks_for_closure.fetch_add(1, Ordering::SeqCst);
                        Ok(attempt == 0)
                    }
                },
                5,
                Duration::from_millis(1),
            )
            .await;

            assert!(result.is_ok(), "wait should stop after rebase clears");
            assert_eq!(checks.load(Ordering::SeqCst), 2);
        });
    }

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
        assert!(
            label.starts_with("session-1-"),
            "expected session-1-<uuid>, got {}",
            label
        );
        assert_eq!(label.len(), "session-1-".len() + 6);
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
                auto_worker_session: false,
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
                auto_worker_session: false,
            },
        ];
        let label = next_session_label(&sessions);
        assert!(
            label.starts_with("session-3-"),
            "expected session-3-<uuid>, got {}",
            label
        );
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
                auto_worker_session: false,
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
                auto_worker_session: false,
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
                auto_worker_session: false,
            },
        ];
        // Max is session-3, so next is session-4
        let label = next_session_label(&sessions);
        assert!(
            label.starts_with("session-4-"),
            "expected session-4-<uuid>, got {}",
            label
        );
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
            auto_worker_session: false,
        }];
        let label = next_session_label(&sessions);
        assert!(
            label.starts_with("session-4-"),
            "expected session-4-<uuid>, got {}",
            label
        );
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

    #[test]
    fn test_scaffold_project_rolls_back_directory_when_github_creation_fails() {
        let base_dir = TempDir::new().unwrap();
        let projects_root = TempDir::new().unwrap();
        let app_state = make_test_state(base_dir.path(), projects_root.path());
        let repo_path = projects_root.path().join("gh-create-failure-test");
        let real_git = real_git_path();

        with_fake_cli_bins(|gh_path, git_path, state_dir| {
            let state_dir_display = state_dir.display().to_string();
            write_fake_command(
                gh_path,
                &format!(
                    "if [ -f \"{state_dir_display}/gh-create-fails\" ]; then\n  echo \"gh create failed\" >&2\n  exit 1\nfi\nif [ \"$1\" = \"repo\" ] && [ \"$2\" = \"create\" ]; then\n  \"{real_git}\" -C \"$PWD\" remote add origin git@github.com:test-owner/gh-create-failure-test.git\n  touch \"{state_dir_display}/remote-created\"\n  exit 0\nfi\necho \"unexpected gh invocation: $*\" >&2\nexit 1"
                ),
            );
            write_fake_command(
                git_path,
                "if [ \"$1\" = \"push\" ]; then\n  exit 0\nfi\necho \"unexpected git invocation: $*\" >&2\nexit 1",
            );
            fs::write(state_dir.join("gh-create-fails"), "")
                .expect("mark first gh create as failed");

            let error = run_async_test(scaffold_project(
                state_from_ref(&app_state),
                "gh-create-failure-test".to_string(),
            ))
            .expect_err("gh create failure should bubble up");
            assert!(error.contains("Failed to create GitHub repo"));
            assert!(
                !repo_path.exists(),
                "repo directory should be removed after gh repo create failure"
            );
            assert!(
                !state_dir.join("remote-created").exists(),
                "gh create failure should not leave remote state behind"
            );

            let stored = app_state
                .storage
                .lock()
                .unwrap()
                .list_projects()
                .expect("list projects after failed scaffold");
            assert!(
                stored.is_empty(),
                "failed scaffold should not persist project state"
            );

            fs::remove_file(state_dir.join("gh-create-fails")).expect("allow gh create retry");

            let project = run_async_test(scaffold_project(
                state_from_ref(&app_state),
                "gh-create-failure-test".to_string(),
            ))
            .expect("retry should succeed after rollback");
            assert_eq!(project.name, "gh-create-failure-test");
            assert!(repo_path.exists(), "retry should recreate repo directory");
            assert!(
                state_dir.join("remote-created").exists(),
                "successful retry should create the remote"
            );
        });
    }

    #[test]
    fn test_scaffold_project_does_not_hold_storage_lock_during_external_publish() {
        let base_dir = TempDir::new().unwrap();
        let projects_root = TempDir::new().unwrap();
        let app_state = Arc::new(make_test_state(base_dir.path(), projects_root.path()));
        let real_git = real_git_path();

        with_fake_cli_bins(|gh_path, git_path, state_dir| {
            let state_dir_display = state_dir.display().to_string();
            write_fake_command(
                gh_path,
                &format!(
                    "if [ \"$1\" = \"repo\" ] && [ \"$2\" = \"create\" ]; then\n  touch \"{state_dir_display}/gh-create-started\"\n  while [ -f \"{state_dir_display}/gh-create-block\" ]; do\n    sleep 0.05\n  done\n  \"{real_git}\" -C \"$PWD\" remote add origin git@github.com:test-owner/lock-scope-test.git\n  exit 0\nfi\necho \"unexpected gh invocation: $*\" >&2\nexit 1"
                ),
            );
            write_fake_command(git_path, "if [ \"$1\" = \"push\" ]; then\n  exit 0\nfi\necho \"unexpected git invocation: $*\" >&2\nexit 1");
            fs::write(state_dir.join("gh-create-block"), "").expect("block gh create");

            let app_state_for_thread = Arc::clone(&app_state);
            let handle = thread::spawn(move || {
                run_async_test(scaffold_project(
                    state_from_ref(app_state_for_thread.as_ref()),
                    "lock-scope-test".to_string(),
                ))
            });

            for _ in 0..100 {
                if state_dir.join("gh-create-started").exists() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            assert!(
                state_dir.join("gh-create-started").exists(),
                "scaffold should reach gh repo create"
            );

            let storage_lock_available = app_state.storage.try_lock().is_ok();

            fs::remove_file(state_dir.join("gh-create-block")).expect("unblock gh create");

            let project = handle
                .join()
                .expect("scaffold thread should not panic")
                .expect("scaffold should succeed after gh unblock");

            assert_eq!(project.name, "lock-scope-test");
            assert!(
                storage_lock_available,
                "storage lock should stay available while external publish is blocked"
            );
        });
    }

    #[test]
    fn test_scaffold_project_rolls_back_if_name_is_claimed_before_final_save() {
        let base_dir = TempDir::new().unwrap();
        let projects_root = TempDir::new().unwrap();
        let app_state = Arc::new(make_test_state(base_dir.path(), projects_root.path()));
        let repo_path = projects_root.path().join("lock-race-test");
        let imported_repo = TempDir::new().unwrap();
        let real_git = real_git_path();

        with_fake_cli_bins(|gh_path, git_path, state_dir| {
            let state_dir_display = state_dir.display().to_string();
            write_fake_command(
                gh_path,
                &format!(
                    "if [ \"$1\" = \"repo\" ] && [ \"$2\" = \"create\" ]; then\n  touch \"{state_dir_display}/gh-create-started\"\n  while [ -f \"{state_dir_display}/gh-create-block\" ]; do\n    sleep 0.05\n  done\n  \"{real_git}\" -C \"$PWD\" remote add origin git@github.com:test-owner/lock-race-test.git\n  touch \"{state_dir_display}/remote-created\"\n  exit 0\nfi\nif [ \"$1\" = \"repo\" ] && [ \"$2\" = \"delete\" ]; then\n  rm -f \"{state_dir_display}/remote-created\"\n  touch \"{state_dir_display}/remote-deleted\"\n  exit 0\nfi\necho \"unexpected gh invocation: $*\" >&2\nexit 1"
                ),
            );
            write_fake_command(git_path, "if [ \"$1\" = \"push\" ]; then\n  exit 0\nfi\necho \"unexpected git invocation: $*\" >&2\nexit 1");
            fs::write(state_dir.join("gh-create-block"), "").expect("block gh create");

            let app_state_for_thread = Arc::clone(&app_state);
            let handle = thread::spawn(move || {
                run_async_test(scaffold_project(
                    state_from_ref(app_state_for_thread.as_ref()),
                    "lock-race-test".to_string(),
                ))
            });

            for _ in 0..100 {
                if state_dir.join("gh-create-started").exists() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            assert!(
                state_dir.join("gh-create-started").exists(),
                "scaffold should reach gh repo create"
            );

            let imported = create_project(
                state_from_ref(app_state.as_ref()),
                "lock-race-test".to_string(),
                imported_repo.path().to_string_lossy().to_string(),
            )
            .expect("concurrent create_project should claim the name");
            assert_eq!(imported.name, "lock-race-test");

            fs::remove_file(state_dir.join("gh-create-block")).expect("unblock gh create");

            let error = handle
                .join()
                .expect("scaffold thread should not panic")
                .expect_err("scaffold should fail once the name is claimed");

            assert!(
                error.contains("A project named 'lock-race-test' already exists"),
                "expected duplicate-name failure, got: {error}"
            );
            assert!(
                !repo_path.exists(),
                "scaffold repo should be rolled back if final save loses the name race"
            );
            assert!(
                state_dir.join("remote-deleted").exists(),
                "remote repo should be deleted when the final save loses the name race"
            );

            let stored = app_state
                .storage
                .lock()
                .unwrap()
                .list_projects()
                .expect("list projects after duplicate-name race");
            assert_eq!(
                stored.projects.len(),
                1,
                "only the competing project should remain after rollback"
            );
            assert_eq!(
                stored.projects[0].repo_path,
                imported_repo.path().to_string_lossy()
            );
        });
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

    #[test]
    fn test_scaffold_project_rolls_back_remote_and_local_state_when_initial_push_fails() {
        let base_dir = TempDir::new().unwrap();
        let projects_root = TempDir::new().unwrap();
        let app_state = make_test_state(base_dir.path(), projects_root.path());
        let repo_path = projects_root.path().join("rollback-test");
        let real_git = real_git_path();

        with_fake_cli_bins(|gh_path, git_path, state_dir| {
            let state_dir_display = state_dir.display().to_string();
            write_fake_command(
                gh_path,
                &format!(
                    "if [ \"$1\" = \"repo\" ] && [ \"$2\" = \"create\" ]; then\n  \"{real_git}\" -C \"$PWD\" remote add origin git@github.com:test-owner/rollback-test.git\n  touch \"{state_dir_display}/remote-created\"\n  exit 0\nfi\nif [ \"$1\" = \"repo\" ] && [ \"$2\" = \"delete\" ]; then\n  if [ \"$3\" != \"test-owner/rollback-test\" ]; then\n    echo \"unexpected repo delete target: $3\" >&2\n    exit 1\n  fi\n  rm -f \"{state_dir_display}/remote-created\"\n  touch \"{state_dir_display}/remote-deleted\"\n  exit 0\nfi\necho \"unexpected gh invocation: $*\" >&2\nexit 1"
                ),
            );
            write_fake_command(
                git_path,
                &format!(
                    "if [ -f \"{state_dir_display}/push-fails\" ]; then\n  echo \"push failed\" >&2\n  exit 1\nfi\nexit 0"
                ),
            );
            fs::write(state_dir.join("push-fails"), "").expect("mark first push as failed");

            let error = run_async_test(scaffold_project(
                state_from_ref(&app_state),
                "rollback-test".to_string(),
            ))
            .expect_err("push failure should bubble up");
            assert!(error.contains("Failed to push initial commit"));
            assert!(
                !repo_path.exists(),
                "repo directory should be removed after push failure"
            );
            assert!(
                state_dir.join("remote-deleted").exists(),
                "remote repo created during scaffold should be deleted on push failure"
            );

            let stored = app_state
                .storage
                .lock()
                .unwrap()
                .list_projects()
                .expect("list projects after failed scaffold");
            assert!(
                stored.projects.is_empty(),
                "failed scaffold should not persist project state"
            );

            fs::remove_file(state_dir.join("push-fails")).expect("allow retry push");
            fs::remove_file(state_dir.join("remote-deleted"))
                .expect("clear previous delete marker");

            let project = run_async_test(scaffold_project(
                state_from_ref(&app_state),
                "rollback-test".to_string(),
            ))
            .expect("retry should succeed after rollback");
            assert_eq!(project.name, "rollback-test");
            assert!(repo_path.exists(), "retry should recreate repo directory");
            assert!(
                state_dir.join("remote-created").exists(),
                "successful retry should recreate the remote"
            );

            let stored = app_state
                .storage
                .lock()
                .unwrap()
                .list_projects()
                .expect("list projects after successful retry");
            assert_eq!(
                stored.projects.len(),
                1,
                "successful retry should persist exactly one project"
            );
            assert_eq!(stored.projects[0].repo_path, repo_path.to_string_lossy());
        });
    }

    #[test]
    fn test_rollback_session_metadata_for_create_session_path() {
        let base_dir = TempDir::new().unwrap();
        let projects_root = TempDir::new().unwrap();
        let repo_dir = TempDir::new().unwrap();
        let app_state = make_test_state(base_dir.path(), projects_root.path());

        let project = create_project(
            state_from_ref(&app_state),
            "rollback-session-create".to_string(),
            repo_dir.path().to_string_lossy().to_string(),
        )
        .expect("create project");

        let session_id = Uuid::new_v4();
        let error = update_project_with_rollback(
            &app_state,
            project.id,
            |project| {
                project.sessions.push(SessionConfig {
                    id: session_id,
                    label: "session-1".to_string(),
                    worktree_path: Some("/tmp/worktree".to_string()),
                    worktree_branch: Some("session-1".to_string()),
                    archived: false,
                    kind: "claude".to_string(),
                    github_issue: None,
                    initial_prompt: None,
                    done_commits: vec![],
                    auto_worker_session: false,
                });
                Ok(())
            },
            |project| {
                project.sessions.retain(|session| session.id != session_id);
                Ok(())
            },
            |()| Err::<(), String>("spawn failed".to_string()),
        )
        .expect_err("post-save failure should bubble up");

        assert_eq!(error, "spawn failed");

        let stored = app_state
            .storage
            .lock()
            .unwrap()
            .load_project(project.id)
            .expect("load project after rollback");
        assert!(
            stored.sessions.is_empty(),
            "failed create-session path should remove persisted session metadata"
        );
    }

    #[test]
    fn test_rollback_session_metadata_preserves_concurrent_project_updates() {
        let base_dir = TempDir::new().unwrap();
        let projects_root = TempDir::new().unwrap();
        let repo_dir = TempDir::new().unwrap();
        let app_state = make_test_state(base_dir.path(), projects_root.path());

        let project = create_project(
            state_from_ref(&app_state),
            "rollback-session-concurrency".to_string(),
            repo_dir.path().to_string_lossy().to_string(),
        )
        .expect("create project");

        let session_id = Uuid::new_v4();
        let prompt_id = Uuid::new_v4();
        let error = update_project_with_rollback(
            &app_state,
            project.id,
            |project| {
                project.sessions.push(SessionConfig {
                    id: session_id,
                    label: "session-1".to_string(),
                    worktree_path: Some("/tmp/worktree".to_string()),
                    worktree_branch: Some("session-1".to_string()),
                    archived: false,
                    kind: "claude".to_string(),
                    github_issue: None,
                    initial_prompt: None,
                    done_commits: vec![],
                    auto_worker_session: false,
                });
                Ok(())
            },
            |project| {
                project.sessions.retain(|session| session.id != session_id);
                Ok(())
            },
            |()| {
                let storage = app_state.storage.lock().unwrap();
                let mut latest = storage
                    .load_project(project.id)
                    .expect("load latest project");
                latest.prompts.push(SavedPrompt {
                    id: prompt_id,
                    name: "Concurrent prompt".to_string(),
                    text: "Preserve me".to_string(),
                    created_at: "2026-03-10T00:00:00Z".to_string(),
                    source_session_label: "session-elsewhere".to_string(),
                });
                storage
                    .save_project(&latest)
                    .expect("save concurrent update");
                Err::<(), String>("spawn failed".to_string())
            },
        )
        .expect_err("post-save failure should bubble up");

        assert_eq!(error, "spawn failed");

        let stored = app_state
            .storage
            .lock()
            .unwrap()
            .load_project(project.id)
            .expect("load project after rollback");
        assert!(
            stored.sessions.is_empty(),
            "rollback should still remove the failed session metadata"
        );
        assert_eq!(stored.prompts.len(), 1);
        assert_eq!(stored.prompts[0].id, prompt_id);
    }

    #[test]
    fn test_cleanup_failed_session_spawn_removes_created_worktree() {
        let repo_dir = TempDir::new().unwrap();
        let repo = git2::Repository::init(repo_dir.path()).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let tree_id = repo.treebuilder(None).unwrap().write().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
            .unwrap();

        let worktree_root = TempDir::new().unwrap();
        let branch = "session-cleanup";
        let worktree_path = WorktreeManager::create_worktree(
            &repo_dir.path().to_string_lossy(),
            branch,
            &worktree_root.path().join(branch),
        )
        .expect("create worktree");

        cleanup_failed_session_spawn(
            &repo_dir.path().to_string_lossy(),
            Some(worktree_path.to_str().unwrap()),
            Some(branch),
        )
        .expect("cleanup created worktree");

        assert!(
            !worktree_path.exists(),
            "failed session spawn cleanup should remove the worktree directory"
        );
        assert!(
            repo.find_branch(branch, git2::BranchType::Local).is_err(),
            "failed session spawn cleanup should remove the branch reference"
        );
    }

    #[test]
    fn test_create_project_succeeds_with_corrupt_sibling_metadata() {
        let base_dir = TempDir::new().unwrap();
        let projects_root = TempDir::new().unwrap();
        let app_state = make_test_state(base_dir.path(), projects_root.path());
        let repo_dir = TempDir::new().unwrap();

        let corrupt_dir = base_dir
            .path()
            .join("projects")
            .join(Uuid::new_v4().to_string());
        fs::create_dir_all(&corrupt_dir).expect("create corrupt dir");
        fs::write(corrupt_dir.join("project.json"), "{ invalid json").expect("write corrupt json");

        let project = create_project(
            state_from_ref(&app_state),
            "fresh-project".to_string(),
            repo_dir.path().to_string_lossy().to_string(),
        )
        .expect("create project should ignore corrupt sibling metadata");

        assert_eq!(project.name, "fresh-project");
    }

    #[test]
    fn test_create_project_rejects_invalid_project_name() {
        let base_dir = TempDir::new().unwrap();
        let projects_root = TempDir::new().unwrap();
        let app_state = make_test_state(base_dir.path(), projects_root.path());
        let repo_dir = TempDir::new().unwrap();

        let error = create_project(
            state_from_ref(&app_state),
            "invalid/name".to_string(),
            repo_dir.path().to_string_lossy().to_string(),
        )
        .expect_err("create_project should reject invalid names");

        assert!(error.contains("Invalid project name: invalid/name"));
    }

    #[test]
    fn test_create_project_rejects_duplicate_name_even_if_existing_project_is_archived() {
        let base_dir = TempDir::new().unwrap();
        let projects_root = TempDir::new().unwrap();
        let app_state = make_test_state(base_dir.path(), projects_root.path());
        let existing_repo = TempDir::new().unwrap();
        let new_repo = TempDir::new().unwrap();

        {
            let storage = app_state.storage.lock().expect("lock storage");
            storage
                .save_project(&Project {
                    id: Uuid::new_v4(),
                    name: "duplicate-name".to_string(),
                    repo_path: existing_repo.path().to_string_lossy().to_string(),
                    created_at: "2026-03-10T00:00:00Z".to_string(),
                    archived: true,
                    maintainer: crate::models::MaintainerConfig::default(),
                    auto_worker: crate::models::AutoWorkerConfig::default(),
                    prompts: vec![],
                    sessions: vec![],
                    staged_session: None,
                })
                .expect("save existing project");
        }

        let error = create_project(
            state_from_ref(&app_state),
            "duplicate-name".to_string(),
            new_repo.path().to_string_lossy().to_string(),
        )
        .expect_err("create_project should reject duplicate names regardless of archived flag");

        assert_eq!(error, "A project named 'duplicate-name' already exists");
    }

    #[test]
    fn test_load_project_succeeds_with_corrupt_sibling_metadata() {
        let base_dir = TempDir::new().unwrap();
        let projects_root = TempDir::new().unwrap();
        let app_state = make_test_state(base_dir.path(), projects_root.path());
        let repo_dir = TempDir::new().unwrap();
        git2::Repository::init(repo_dir.path()).expect("init git repo");

        let corrupt_dir = base_dir
            .path()
            .join("projects")
            .join(Uuid::new_v4().to_string());
        fs::create_dir_all(&corrupt_dir).expect("create corrupt dir");
        fs::write(corrupt_dir.join("project.json"), "{ invalid json").expect("write corrupt json");

        let project = load_project(
            state_from_ref(&app_state),
            "imported-project".to_string(),
            repo_dir.path().to_string_lossy().to_string(),
        )
        .expect("load project should ignore corrupt sibling metadata");

        assert_eq!(project.name, "imported-project");
    }

    #[test]
    fn test_load_project_rejects_invalid_project_name() {
        let base_dir = TempDir::new().unwrap();
        let projects_root = TempDir::new().unwrap();
        let app_state = make_test_state(base_dir.path(), projects_root.path());
        let repo_dir = TempDir::new().unwrap();
        git2::Repository::init(repo_dir.path()).expect("init git repo");

        let error = load_project(
            state_from_ref(&app_state),
            "invalid/name".to_string(),
            repo_dir.path().to_string_lossy().to_string(),
        )
        .expect_err("load_project should reject invalid names");

        assert!(error.contains("Invalid project name: invalid/name"));
    }

    #[test]
    fn test_list_projects_includes_projects_marked_archived_in_storage() {
        let base_dir = TempDir::new().unwrap();
        let projects_root = TempDir::new().unwrap();
        let app_state = make_test_state(base_dir.path(), projects_root.path());

        {
            let storage = app_state.storage.lock().expect("lock storage");
            storage
                .save_project(&Project {
                    id: Uuid::new_v4(),
                    name: "stored-project".to_string(),
                    repo_path: projects_root.path().join("stored-project").to_string_lossy().to_string(),
                    created_at: "2026-03-10T00:00:00Z".to_string(),
                    archived: true,
                    maintainer: crate::models::MaintainerConfig::default(),
                    auto_worker: crate::models::AutoWorkerConfig::default(),
                    prompts: vec![],
                    sessions: vec![],
                    staged_session: None,
                })
                .expect("save archived-flagged project");
        }

        let inventory = list_projects(state_from_ref(&app_state)).expect("list projects");

        assert_eq!(inventory.projects.len(), 1);
        assert_eq!(inventory.projects[0].name, "stored-project");
    }

    #[test]
    fn test_scaffold_project_succeeds_with_corrupt_sibling_metadata() {
        let base_dir = TempDir::new().unwrap();
        let projects_root = TempDir::new().unwrap();
        let app_state = make_test_state(base_dir.path(), projects_root.path());
        let repo_path = projects_root.path().join("scaffold-with-corrupt-sibling");
        let real_git = real_git_path();

        let corrupt_dir = base_dir
            .path()
            .join("projects")
            .join(Uuid::new_v4().to_string());
        fs::create_dir_all(&corrupt_dir).expect("create corrupt dir");
        fs::write(corrupt_dir.join("project.json"), "{ invalid json").expect("write corrupt json");

        with_fake_cli_bins(|gh_path, git_path, _state_dir| {
            write_fake_command(
                gh_path,
                &format!(
                    "if [ \"$1\" = \"repo\" ] && [ \"$2\" = \"create\" ]; then\n  \"{real_git}\" -C \"$PWD\" remote add origin git@github.com:test-owner/scaffold-with-corrupt-sibling.git\n  exit 0\nfi\necho \"unexpected gh invocation: $*\" >&2\nexit 1"
                ),
            );
            write_fake_command(git_path, "exit 0");

            let project = run_async_test(scaffold_project(
                state_from_ref(&app_state),
                "scaffold-with-corrupt-sibling".to_string(),
            ))
            .expect("scaffold should ignore corrupt sibling metadata");

            assert_eq!(project.name, "scaffold-with-corrupt-sibling");
            assert!(repo_path.exists(), "repo should be created");
        });
    }

    // --- validate_maintainer_interval tests ---

    #[test]
    fn test_validate_interval_minutes() {
        assert!(validate_maintainer_interval(5).is_ok());
        assert!(validate_maintainer_interval(60).is_ok());
        assert!(validate_maintainer_interval(1440).is_ok());
        assert!(validate_maintainer_interval(0).is_err());
        assert!(validate_maintainer_interval(4).is_err());
    }

    #[test]
    fn test_trigger_maintainer_check_uses_spawn_blocking() {
        let source =
            fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands.rs"))
                .expect("read commands source");
        let start = source
            .find("pub async fn trigger_maintainer_check")
            .expect("find trigger_maintainer_check");
        let rest = &source[start..];
        let end = rest
            .find("\n#[tauri::command]")
            .expect("find end of trigger_maintainer_check");
        let function_body = &rest[..end];

        assert!(
            function_body.contains("spawn_blocking"),
            "trigger_maintainer_check must offload blocking maintainer work with spawn_blocking"
        );
    }

    #[test]
    fn test_run_maintainer_check_spawn_blocking_with_offloads_work() {
        run_async_test(async {
            let runtime_thread_id = thread::current().id();
            let project_id = Uuid::new_v4();

            let log = run_maintainer_check_spawn_blocking_with(
                "/tmp/project".to_string(),
                project_id,
                Some("owner/repo".to_string()),
                move |repo_path, inner_project_id, github_repo| {
                    assert_eq!(repo_path, "/tmp/project");
                    assert_eq!(inner_project_id, project_id);
                    assert_eq!(github_repo.as_deref(), Some("owner/repo"));
                    assert_ne!(thread::current().id(), runtime_thread_id);

                    Ok(MaintainerRunLog {
                        id: Uuid::new_v4(),
                        project_id: inner_project_id,
                        timestamp: "2026-03-10T00:00:00Z".to_string(),
                        issues_filed: vec![],
                        issues_updated: vec![],
                        issues_unchanged: 0,
                        issues_skipped: 0,
                        summary: "No actionable maintainer issues found".to_string(),
                    })
                },
            )
            .await
            .expect("maintainer check should succeed");

            assert_eq!(log.project_id, project_id);
        });
    }

    fn make_issue(number: u64, title: &str, labels: &[&str]) -> crate::models::GithubIssue {
        crate::models::GithubIssue {
            number,
            title: title.to_string(),
            url: format!("https://github.com/example/repo/issues/{number}"),
            body: Some(format!("Body for issue {number}")),
            labels: labels
                .iter()
                .map(|label| crate::models::GithubLabel {
                    name: (*label).to_string(),
                })
                .collect(),
        }
    }

    #[test]
    fn test_build_auto_worker_queue_filters_and_pins_active_issue() {
        let issues = vec![
            make_issue(11, "Eligible queued issue", &["priority:high", "complexity:low"]),
            make_issue(12, "Busy issue", &["priority:high", "complexity:low", "in-progress"]),
        ];
        let active_issue = Some(make_issue(
            12,
            "Busy issue",
            &["priority:high", "complexity:low", "in-progress", "assigned-to-auto-worker"],
        ));

        let queue = build_auto_worker_queue(issues, active_issue);

        assert_eq!(queue.len(), 2);
        assert_eq!(queue[0].number, 12);
        assert!(queue[0].is_active);
        assert_eq!(queue[1].number, 11);
        assert!(!queue[1].is_active);
    }

    #[test]
    fn test_build_auto_worker_queue_avoids_duplicate_active_issue() {
        let issues = vec![make_issue(21, "Already active", &["priority:high", "complexity:low"])];
        let active_issue = Some(make_issue(21, "Already active", &["priority:high", "complexity:low"]));

        let queue = build_auto_worker_queue(issues, active_issue);

        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].number, 21);
        assert!(queue[0].is_active);
    }

    #[test]
    fn test_get_auto_worker_queue_uses_cached_issues() {
        let base_dir = TempDir::new().unwrap();
        let projects_root = TempDir::new().unwrap();
        let app_state = make_test_state(base_dir.path(), projects_root.path());
        let project_id = Uuid::new_v4();
        let repo_path = projects_root.path().join("queue-project");
        fs::create_dir_all(&repo_path).expect("create repo dir");

        {
            let storage = app_state.storage.lock().expect("lock storage");
            storage
                .save_project(&crate::models::Project {
                    id: project_id,
                    name: "Queue Project".to_string(),
                    repo_path: repo_path.to_string_lossy().to_string(),
                    created_at: "2026-03-10T00:00:00Z".to_string(),
                    archived: false,
                    sessions: vec![crate::models::SessionConfig {
                        id: Uuid::new_v4(),
                        label: "session-1".to_string(),
                        worktree_path: None,
                        worktree_branch: None,
                        archived: false,
                        kind: "claude".to_string(),
                        github_issue: Some(make_issue(
                            33,
                            "Active worker issue",
                            &["priority:high", "complexity:low", "assigned-to-auto-worker"],
                        )),
                        initial_prompt: None,
                        done_commits: vec![],
                        auto_worker_session: true,
                    }],
                    maintainer: crate::models::MaintainerConfig::default(),
                    auto_worker: crate::models::AutoWorkerConfig { enabled: true },
                    prompts: vec![],
                    staged_session: None,
                })
                .expect("save project");
        }

        {
            let mut issue_cache = app_state.issue_cache.lock().expect("lock cache");
            issue_cache.insert(
                repo_path.to_string_lossy().to_string(),
                vec![
                    make_issue(33, "Active worker issue", &["priority:high", "complexity:low"]),
                    make_issue(34, "Queued issue", &["priority:high", "complexity:low"]),
                    make_issue(35, "Ineligible issue", &["priority:high"]),
                ],
            );
        }

        let queue = run_async_test(get_auto_worker_queue(
            state_from_ref(&app_state),
            project_id.to_string(),
        ))
        .expect("queue command should succeed");

        assert_eq!(queue.len(), 2);
        assert_eq!(queue[0].number, 33);
        assert!(queue[0].is_active);
        assert_eq!(queue[1].number, 34);
        assert!(!queue[1].is_active);
    }

    #[test]
    fn test_submit_secure_env_value_command_writes_env_file() {
        let base_dir = TempDir::new().unwrap();
        let projects_root = TempDir::new().unwrap();
        let repo_dir = TempDir::new().unwrap();
        let app_state = make_test_state(base_dir.path(), projects_root.path());

        let project = create_project(
            state_from_ref(&app_state),
            "secure-env-submit".to_string(),
            repo_dir.path().to_string_lossy().to_string(),
        )
        .expect("create project");

        crate::secure_env::begin_secure_env_request(
            &app_state,
            &project.name,
            "OPENAI_API_KEY",
            "req-123",
        )
        .expect("begin secure env request");

        let status = run_async_test(submit_secure_env_value(
            state_from_ref(&app_state),
            "req-123".to_string(),
            "new-secret".to_string(),
        ))
        .expect("submit secure env value");

        assert_eq!(status, "created");
        let written = fs::read_to_string(repo_dir.path().join(".env")).expect("read .env");
        assert_eq!(written, "OPENAI_API_KEY=new-secret\n");
    }

    #[test]
    fn test_cancel_secure_env_request_command_clears_pending_request() {
        let base_dir = TempDir::new().unwrap();
        let projects_root = TempDir::new().unwrap();
        let repo_dir = TempDir::new().unwrap();
        let app_state = make_test_state(base_dir.path(), projects_root.path());

        let project = create_project(
            state_from_ref(&app_state),
            "secure-env-cancel".to_string(),
            repo_dir.path().to_string_lossy().to_string(),
        )
        .expect("create project");

        crate::secure_env::begin_secure_env_request(
            &app_state,
            &project.name,
            "OPENAI_API_KEY",
            "req-123",
        )
        .expect("begin secure env request");

        cancel_secure_env_request(
            state_from_ref(&app_state),
            "req-123".to_string(),
        )
        .expect("cancel secure env request");

        assert!(app_state.secure_env_request.lock().unwrap().is_none());
    }
}

#[cfg(test)]
mod staging_tests {
    use super::*;

    #[test]
    fn test_find_free_port_returns_offset_port_when_free() {
        // Port 59123 is unlikely to be in use
        let port = find_staging_port(58123).unwrap();
        assert_eq!(port, 59123); // base + 1000
    }

    #[test]
    fn test_find_free_port_skips_occupied() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let occupied_port = listener.local_addr().unwrap().port();
        let Some(base) = occupied_port.checked_sub(STAGING_PORT_OFFSET) else {
            return; // OS assigned a port too low to construct a valid base
        };
        let port = find_staging_port(base).unwrap();
        assert!(port > occupied_port);
        assert!(port <= occupied_port + 100);
    }

    #[test]
    fn test_find_staging_port_checks_ipv6() {
        // Bind on IPv6 only — find_staging_port must detect this.
        // Skip gracefully on systems without IPv6 loopback.
        let Ok(listener) = std::net::TcpListener::bind("[::1]:0") else {
            return;
        };
        let occupied_port = listener.local_addr().unwrap().port();
        let base = occupied_port.checked_sub(STAGING_PORT_OFFSET).unwrap();
        let port = find_staging_port(base).unwrap();
        assert_ne!(port, occupied_port, "must skip port occupied on IPv6");
    }
}
