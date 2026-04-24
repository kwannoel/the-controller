use axum::{
    extract::{
        ws::{Message, WebSocket},
        State as AxumState, WebSocketUpgrade,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::Value;
use std::sync::Arc;
use the_controller_lib::{commands, config, emitter::WsBroadcastEmitter, state::AppState};

use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

struct ServerState {
    app: AppState,
    ws_tx: broadcast::Sender<String>,
}

#[tokio::main]
async fn main() {
    let (emitter, ws_tx) = WsBroadcastEmitter::new();
    let app_state = AppState::new(emitter).expect("Failed to initialize app state");

    let state = Arc::new(ServerState {
        app: app_state,
        ws_tx,
    });

    let app = Router::new()
        .route("/api/list_projects", post(list_projects))
        .route("/api/check_onboarding", post(check_onboarding))
        .route("/api/restore_sessions", post(restore_sessions))
        .route("/api/connect_session", post(connect_session))
        .route("/api/load_project", post(load_project))
        .route("/api/create_project", post(create_project))
        .route("/api/delete_project", post(delete_project))
        .route("/api/write_to_pty", post(write_to_pty))
        .route("/api/send_raw_to_pty", post(send_raw_to_pty))
        .route("/api/resize_pty", post(resize_pty))
        .route("/api/close_session", post(close_session))
        .route("/api/create_session", post(create_session))
        .route("/api/list_archived_projects", post(list_archived_projects))
        .route("/api/merge_session_branch", post(merge_session_branch))
        .route("/api/read_daemon_token", post(read_daemon_token))
        .route("/api/log_frontend_error", post(log_frontend_error))
        .route("/api/get_agents_md", post(get_agents_md))
        .route("/api/update_agents_md", post(update_agents_md))
        .route("/api/set_initial_prompt", post(set_initial_prompt))
        .route("/api/save_session_prompt", post(save_session_prompt))
        .route("/api/list_project_prompts", post(list_project_prompts))
        .route("/api/get_session_commits", post(get_session_commits))
        .route(
            "/api/get_session_token_usage",
            post(get_session_token_usage),
        )
        .route("/api/get_repo_head", post(get_repo_head))
        .route("/api/save_onboarding_config", post(save_onboarding_config))
        .route("/api/home_dir", post(home_dir))
        .route("/api/list_directories_at", post(list_directories_at))
        .route("/api/list_root_directories", post(list_root_directories))
        .route("/api/check_claude_cli", post(check_claude_cli))
        .route("/api/generate_project_names", post(generate_project_names))
        .route("/api/list_github_issues", post(list_github_issues))
        .route("/api/create_github_issue", post(create_github_issue))
        .route("/api/close_github_issue", post(close_github_issue))
        .route("/api/delete_github_issue", post(delete_github_issue))
        .route("/api/post_github_comment", post(post_github_comment))
        .route("/api/add_github_label", post(add_github_label))
        .route("/api/remove_github_label", post(remove_github_label))
        .route("/api/generate_issue_body", post(generate_issue_body))
        .route("/api/list_assigned_issues", post(list_assigned_issues))
        .route("/api/kanban_load_order", post(kanban_load_order))
        .route("/api/kanban_save_order", post(kanban_save_order))
        .route("/api/configure_maintainer", post(configure_maintainer))
        .route("/api/configure_auto_worker", post(configure_auto_worker))
        .route("/api/get_worker_reports", post(get_worker_reports))
        .route("/api/get_auto_worker_queue", post(get_auto_worker_queue))
        .route("/api/get_maintainer_status", post(get_maintainer_status))
        .route("/api/get_maintainer_history", post(get_maintainer_history))
        .route(
            "/api/trigger_maintainer_check",
            post(trigger_maintainer_check),
        )
        .route(
            "/api/clear_maintainer_reports",
            post(clear_maintainer_reports),
        )
        .route("/api/get_maintainer_issues", post(get_maintainer_issues))
        .route(
            "/api/get_maintainer_issue_detail",
            post(get_maintainer_issue_detail),
        )
        .route(
            "/api/submit_secure_env_value",
            post(submit_secure_env_value),
        )
        .route(
            "/api/cancel_secure_env_request",
            post(cancel_secure_env_request),
        )
        .route("/api/start_claude_login", post(start_claude_login))
        .route("/api/stop_claude_login", post(stop_claude_login))
        .route("/api/scaffold_project", post(scaffold_project))
        .route("/api/stage_session", post(stage_session))
        .route("/api/unstage_session", post(unstage_session))
        .route("/api/save_screenshot", post(save_screenshot))
        .route("/ws", get(ws_upgrade))
        .fallback(fallback_handler)
        .layer(CorsLayer::permissive())
        .with_state(state);

    let port: u16 = match std::env::var("PORT") {
        Ok(val) => val.parse().unwrap_or_else(|_| {
            eprintln!("Invalid PORT value '{}', must be a u16", val);
            std::process::exit(1);
        }),
        Err(_) => 3001,
    };
    println!("Server listening on http://localhost:{}", port);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn fallback_handler(req: axum::http::Request<axum::body::Body>) -> (StatusCode, String) {
    (
        StatusCode::NOT_FOUND,
        format!("Unknown route: {}", req.uri().path()),
    )
}

// --- Route handlers ---

async fn list_projects(
    AxumState(state): AxumState<Arc<ServerState>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let storage = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let inventory = storage
        .list_projects()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::to_value(inventory).unwrap()))
}

async fn check_onboarding(
    AxumState(state): AxumState<Arc<ServerState>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let storage = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let base_dir = storage.base_dir();
    let cfg = config::load_config(&base_dir);
    Ok(Json(serde_json::to_value(cfg).unwrap()))
}

async fn restore_sessions(
    AxumState(state): AxumState<Arc<ServerState>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let storage = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let inventory = storage
        .list_projects()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    inventory.warn_if_corrupt("restore_sessions");
    for project in &inventory.projects {
        if let Err(e) = storage.migrate_worktree_paths(project) {
            eprintln!(
                "Failed to migrate worktrees for project '{}': {}",
                project.name, e
            );
        }
    }
    Ok(Json(Value::Null))
}

async fn connect_session(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let session_id = args["sessionId"].as_str().unwrap_or_default();
    let rows = args["rows"].as_u64().unwrap_or(24) as u16;
    let cols = args["cols"].as_u64().unwrap_or(80) as u16;
    let id =
        uuid::Uuid::parse_str(session_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    // Check if already connected
    {
        let pty_manager = state
            .app
            .pty_manager
            .lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        if pty_manager.session_ids().contains(&id) {
            return Ok(Json(Value::Null));
        }
    }

    // Find session config from storage
    let (session_dir, kind) = {
        let storage = state
            .app
            .storage
            .lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let inventory = storage
            .list_projects()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
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
            .ok_or_else(|| {
                (
                    StatusCode::NOT_FOUND,
                    format!("session not found: {}", session_id),
                )
            })?
    };

    let pty_manager = state.app.pty_manager.clone();
    let emitter = state.app.emitter.clone();
    tokio::task::spawn_blocking(move || {
        let mut mgr = pty_manager
            .lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        mgr.spawn_session(id, &session_dir, &kind, emitter, true, None, rows, cols)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task failed: {}", e),
        )
    })??;

    Ok(Json(Value::Null))
}

async fn load_project(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let name = args["name"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "name required".to_string()))?
        .to_string();
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "repoPath required".to_string()))?
        .to_string();
    let state = state.clone();
    let project = tokio::task::spawn_blocking(move || {
        commands::load_project_impl(&state.app, name, repo_path)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task failed: {}", e),
        )
    })?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(project).unwrap()))
}

async fn create_project(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let name = args["name"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "name required".to_string()))?
        .to_string();
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "repoPath required".to_string()))?
        .to_string();
    let state = state.clone();
    let project = tokio::task::spawn_blocking(move || {
        commands::create_project_impl(&state.app, name, repo_path)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task failed: {}", e),
        )
    })?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(project).unwrap()))
}

async fn delete_project(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "projectId required".to_string()))?
        .to_string();
    let delete_repo = args["deleteRepo"].as_bool().unwrap_or(false);
    let state = state.clone();
    tokio::task::spawn_blocking(move || {
        commands::delete_project_impl(&state.app, project_id, delete_repo)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task failed: {}", e),
        )
    })?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

async fn write_to_pty(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let session_id = args["sessionId"].as_str().unwrap_or_default();
    let data = args["data"].as_str().unwrap_or_default();
    let id =
        uuid::Uuid::parse_str(session_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let mut pty = state
        .app
        .pty_manager
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    pty.write_to_session(id, data.as_bytes())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

async fn send_raw_to_pty(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let session_id = args["sessionId"].as_str().unwrap_or_default();
    let data = args["data"].as_str().unwrap_or_default();
    let id =
        uuid::Uuid::parse_str(session_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let mut pty = state
        .app
        .pty_manager
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    pty.send_raw_to_session(id, data.as_bytes())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

async fn resize_pty(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let session_id = args["sessionId"].as_str().unwrap_or_default();
    let rows = args["rows"].as_u64().unwrap_or(24) as u16;
    let cols = args["cols"].as_u64().unwrap_or(80) as u16;
    let id =
        uuid::Uuid::parse_str(session_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let pty = state
        .app
        .pty_manager
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    pty.resize_session(id, rows, cols)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

async fn close_session(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "projectId required".to_string()))?
        .to_string();
    let session_id = args["sessionId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "sessionId required".to_string()))?
        .to_string();
    let delete_worktree = args["deleteWorktree"].as_bool().unwrap_or(false);
    let state = state.clone();
    tokio::task::spawn_blocking(move || {
        commands::close_session_impl(&state.app, project_id, session_id, delete_worktree)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task failed: {}", e),
        )
    })?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

async fn create_session(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "projectId required".to_string()))?
        .to_string();
    let kind = args["kind"].as_str().map(str::to_string);
    let background = args["background"].as_bool();
    let initial_prompt = args["initialPrompt"].as_str().map(str::to_string);
    let github_issue = if args["githubIssue"].is_null() {
        None
    } else {
        Some(
            serde_json::from_value(args["githubIssue"].clone())
                .map_err(|e| (StatusCode::BAD_REQUEST, format!("githubIssue: {}", e)))?,
        )
    };

    let state = state.clone();
    let session_id = tokio::task::spawn_blocking(move || {
        commands::create_session_impl(
            &state.app,
            project_id,
            kind,
            github_issue,
            background,
            initial_prompt,
        )
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task failed: {}", e),
        )
    })?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(Value::String(session_id)))
}

async fn read_daemon_token() -> Result<Json<Value>, (StatusCode, String)> {
    let token = commands::daemon::read_daemon_token()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::String(token)))
}

async fn log_frontend_error(Json(args): Json<Value>) -> Result<Json<Value>, (StatusCode, String)> {
    let message = args["message"].as_str().unwrap_or("");
    eprintln!("[FRONTEND] {}", message);
    Ok(Json(Value::Null))
}

async fn list_archived_projects(
    AxumState(state): AxumState<Arc<ServerState>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let storage = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let inventory = storage
        .list_projects()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let filtered = inventory.filter_projects(|project| {
        project.archived || project.sessions.iter().any(|session| session.archived)
    });
    Ok(Json(serde_json::to_value(filtered).unwrap()))
}
async fn merge_session_branch(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    use the_controller_lib::models::MergeResponse;
    use the_controller_lib::worktree::{MergeResult, WorktreeManager};

    let project_id = args["projectId"].as_str().unwrap_or_default();
    let session_id = args["sessionId"].as_str().unwrap_or_default();
    let project_uuid =
        uuid::Uuid::parse_str(project_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let session_uuid =
        uuid::Uuid::parse_str(session_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let (repo_path, worktree_path, branch_name) = {
        let storage = state
            .app
            .storage
            .lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let project = storage
            .load_project(project_uuid)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let session = project
            .sessions
            .iter()
            .find(|s| s.id == session_uuid)
            .ok_or_else(|| (StatusCode::NOT_FOUND, "Session not found".to_string()))?;
        let wt_path = session.worktree_path.clone().ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "Session has no worktree".to_string(),
            )
        })?;
        let branch = session
            .worktree_branch
            .clone()
            .ok_or_else(|| (StatusCode::BAD_REQUEST, "Session has no branch".to_string()))?;
        (project.repo_path.clone(), wt_path, branch)
    };

    const MAX_RETRIES: u32 = 5;
    const POLL_INTERVAL_SECS: u64 = 3;

    for attempt in 0..MAX_RETRIES {
        let rp = repo_path.clone();
        let wt = worktree_path.clone();
        let br = branch_name.clone();

        let result = tokio::task::spawn_blocking(move || {
            if WorktreeManager::is_rebase_in_progress(&wt) {
                Ok(MergeResult::RebaseConflicts)
            } else {
                WorktreeManager::merge_via_pr(&rp, &wt, &br)
            }
        })
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Task failed: {}", e),
            )
        })?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

        match result {
            MergeResult::PrCreated(url) => {
                let resp = MergeResponse::PrCreated { url };
                return Ok(Json(serde_json::to_value(resp).unwrap()));
            }
            MergeResult::RebaseConflicts => {
                let prompt = "merge\r";
                {
                    let mut pty_manager = state
                        .app
                        .pty_manager
                        .lock()
                        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                    let _ = pty_manager.write_to_session(session_uuid, prompt.as_bytes());
                }

                let _ = state.app.emitter.emit(
                    "merge-status",
                    &format!(
                        "Rebase conflicts (attempt {}/{}). Claude is resolving...",
                        attempt + 1,
                        MAX_RETRIES
                    ),
                );

                let wt_poll = worktree_path.clone();
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
                    let wt_check = wt_poll.clone();
                    let still_rebasing = tokio::task::spawn_blocking(move || {
                        WorktreeManager::is_rebase_in_progress(&wt_check)
                    })
                    .await
                    .map_err(|e| {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("Task failed: {}", e),
                        )
                    })?;
                    if !still_rebasing {
                        break;
                    }
                }
                continue;
            }
        }
    }

    Err((
        StatusCode::INTERNAL_SERVER_ERROR,
        format!(
            "Merge failed after {} attempts due to recurring conflicts",
            MAX_RETRIES
        ),
    ))
}

async fn get_agents_md(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "projectId required".to_string()))?
        .to_string();
    let content = commands::get_agents_md_impl(&state.app, project_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::String(content)))
}

async fn update_agents_md(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "projectId required".to_string()))?
        .to_string();
    let content = args["content"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "content required".to_string()))?
        .to_string();
    commands::update_agents_md_impl(&state.app, project_id, content)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

async fn set_initial_prompt(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "projectId required".to_string()))?
        .to_string();
    let session_id = args["sessionId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "sessionId required".to_string()))?
        .to_string();
    let prompt = args["prompt"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "prompt required".to_string()))?
        .to_string();
    commands::set_initial_prompt_impl(&state.app, project_id, session_id, prompt)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

async fn save_session_prompt(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "projectId required".to_string()))?
        .to_string();
    let session_id = args["sessionId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "sessionId required".to_string()))?
        .to_string();
    commands::save_session_prompt_impl(&state.app, project_id, session_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

async fn list_project_prompts(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "projectId required".to_string()))?
        .to_string();
    let prompts = commands::list_project_prompts_impl(&state.app, project_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(prompts).unwrap()))
}

async fn get_session_commits(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "projectId required".to_string()))?
        .to_string();
    let session_id = args["sessionId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "sessionId required".to_string()))?
        .to_string();
    let state = state.clone();
    let commits = tokio::task::spawn_blocking(move || {
        commands::get_session_commits_impl(&state.app, project_id, session_id)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task failed: {}", e),
        )
    })?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(commits).unwrap()))
}

async fn get_session_token_usage(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "projectId required".to_string()))?
        .to_string();
    let session_id = args["sessionId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "sessionId required".to_string()))?
        .to_string();
    let usage = commands::get_session_token_usage_impl(&state.app, project_id, session_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(usage).unwrap()))
}

async fn get_repo_head(Json(args): Json<Value>) -> Result<Json<Value>, (StatusCode, String)> {
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "repoPath required".to_string()))?
        .to_string();
    let (branch, short_hash) =
        tokio::task::spawn_blocking(move || commands::get_repo_head_impl(repo_path))
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Task failed: {}", e),
                )
            })?
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::json!([branch, short_hash])))
}

async fn save_onboarding_config(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let projects_root = args["projectsRoot"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "projectsRoot required".to_string()))?
        .to_string();
    commands::save_onboarding_config_impl(&state.app, projects_root)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

async fn home_dir() -> Result<Json<Value>, (StatusCode, String)> {
    let home = dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not determine home directory".to_string(),
            )
        })?;
    Ok(Json(Value::String(home)))
}

async fn list_directories_at(Json(args): Json<Value>) -> Result<Json<Value>, (StatusCode, String)> {
    let path = args["path"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "path required".to_string()))?
        .to_string();
    let p = std::path::PathBuf::from(&path);
    if !p.is_dir() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Not a directory: {}", path),
        ));
    }
    let entries = config::list_directories(&p)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::to_value(entries).unwrap()))
}

async fn list_root_directories(
    AxumState(state): AxumState<Arc<ServerState>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let entries = commands::list_root_directories_impl(&state.app)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(entries).unwrap()))
}

async fn check_claude_cli() -> Result<Json<Value>, (StatusCode, String)> {
    let result = tokio::task::spawn_blocking(config::check_claude_cli_status)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Task failed: {}", e),
            )
        })?;
    Ok(Json(Value::String(result)))
}

async fn generate_project_names(
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let description = args["description"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "description required".to_string()))?
        .to_string();
    let names = tokio::task::spawn_blocking(move || config::generate_names_via_cli(&description))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Task failed: {}", e),
            )
        })?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(names).unwrap()))
}

// --- GitHub ---

async fn list_github_issues(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "repoPath required".to_string()))?
        .to_string();
    let issues = commands::github::list_github_issues(repo_path, &state.app)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(issues).unwrap()))
}

async fn create_github_issue(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "repoPath required".to_string()))?
        .to_string();
    let title = args["title"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "title required".to_string()))?
        .to_string();
    let body = args["body"].as_str().unwrap_or("").to_string();
    let issue = commands::github::create_github_issue(&state.app, repo_path, title, body)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(issue).unwrap()))
}

async fn close_github_issue(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "repoPath required".to_string()))?
        .to_string();
    let issue_number = args["issueNumber"]
        .as_u64()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "issueNumber required".to_string()))?;
    let comment = args["comment"].as_str().unwrap_or("").to_string();
    commands::github::close_github_issue(&state.app, repo_path, issue_number, comment)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

async fn delete_github_issue(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "repoPath required".to_string()))?
        .to_string();
    let issue_number = args["issueNumber"]
        .as_u64()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "issueNumber required".to_string()))?;
    commands::github::delete_github_issue(&state.app, repo_path, issue_number)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

async fn post_github_comment(Json(args): Json<Value>) -> Result<Json<Value>, (StatusCode, String)> {
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "repoPath required".to_string()))?
        .to_string();
    let issue_number = args["issueNumber"]
        .as_u64()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "issueNumber required".to_string()))?;
    let body = args["body"].as_str().unwrap_or("").to_string();
    commands::github::post_github_comment(repo_path, issue_number, body)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

async fn add_github_label(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "repoPath required".to_string()))?
        .to_string();
    let issue_number = args["issueNumber"]
        .as_u64()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "issueNumber required".to_string()))?;
    let label = args["label"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "label required".to_string()))?
        .to_string();
    let description = args["description"].as_str().map(str::to_string);
    let color = args["color"].as_str().map(str::to_string);
    commands::github::add_github_label(
        &state.app,
        repo_path,
        issue_number,
        label,
        description,
        color,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

async fn remove_github_label(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "repoPath required".to_string()))?
        .to_string();
    let issue_number = args["issueNumber"]
        .as_u64()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "issueNumber required".to_string()))?;
    let label = args["label"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "label required".to_string()))?
        .to_string();
    commands::github::remove_github_label(&state.app, repo_path, issue_number, label)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

async fn generate_issue_body(Json(args): Json<Value>) -> Result<Json<Value>, (StatusCode, String)> {
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "repoPath required".to_string()))?
        .to_string();
    let title = args["title"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "title required".to_string()))?
        .to_string();
    let body = commands::github::generate_issue_body(repo_path, title)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::String(body)))
}

async fn list_assigned_issues(
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "repoPath required".to_string()))?
        .to_string();
    let issues = commands::github::list_assigned_issues(repo_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(issues).unwrap()))
}

// --- Kanban ---

fn kanban_order_file(state: &AppState) -> Result<std::path::PathBuf, (StatusCode, String)> {
    let storage = state
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(commands::kanban::order_file_in(&storage.base_dir()))
}

async fn kanban_load_order(
    AxumState(state): AxumState<Arc<ServerState>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let path = kanban_order_file(&state.app)?;
    let order = tokio::task::spawn_blocking(move || commands::kanban::load_order_from(&path))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Task failed: {}", e),
            )
        })?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(order))
}

async fn kanban_save_order(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let order = args["order"].clone();
    let path = kanban_order_file(&state.app)?;
    tokio::task::spawn_blocking(move || commands::kanban::save_order_to(&path, &order))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Task failed: {}", e),
            )
        })?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

// --- Maintainer / auto-worker ---

async fn configure_maintainer(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "projectId required".to_string()))?
        .to_string();
    let enabled = args["enabled"].as_bool().unwrap_or(false);
    let interval_minutes = args["intervalMinutes"].as_u64().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "intervalMinutes required".to_string(),
        )
    })?;
    let github_repo = args["githubRepo"].as_str().map(str::to_string);
    commands::configure_maintainer_impl(
        &state.app,
        project_id,
        enabled,
        interval_minutes,
        github_repo,
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

async fn configure_auto_worker(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "projectId required".to_string()))?
        .to_string();
    let enabled = args["enabled"].as_bool().unwrap_or(false);
    commands::configure_auto_worker_impl(&state.app, project_id, enabled)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

async fn get_worker_reports(Json(args): Json<Value>) -> Result<Json<Value>, (StatusCode, String)> {
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "repoPath required".to_string()))?
        .to_string();
    let reports = commands::github::get_worker_reports(repo_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(reports).unwrap()))
}

async fn get_auto_worker_queue(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "projectId required".to_string()))?
        .to_string();
    let queue = commands::get_auto_worker_queue_impl(&state.app, project_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(queue).unwrap()))
}

async fn get_maintainer_status(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "projectId required".to_string()))?
        .to_string();
    let status = commands::get_maintainer_status_impl(&state.app, project_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(status).unwrap()))
}

async fn get_maintainer_history(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "projectId required".to_string()))?
        .to_string();
    let history = commands::get_maintainer_history_impl(&state.app, project_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(history).unwrap()))
}

async fn trigger_maintainer_check(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "projectId required".to_string()))?
        .to_string();
    let log = commands::trigger_maintainer_check_impl(&state.app, project_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(log).unwrap()))
}

async fn clear_maintainer_reports(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "projectId required".to_string()))?
        .to_string();
    commands::clear_maintainer_reports_impl(&state.app, project_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

async fn get_maintainer_issues(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "projectId required".to_string()))?
        .to_string();
    let issues = commands::get_maintainer_issues_impl(&state.app, project_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(issues).unwrap()))
}

async fn get_maintainer_issue_detail(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "projectId required".to_string()))?
        .to_string();
    let issue_number = args["issueNumber"]
        .as_u64()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "issueNumber required".to_string()))?
        as u32;
    let detail = commands::get_maintainer_issue_detail_impl(&state.app, project_id, issue_number)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(detail).unwrap()))
}

// --- Secure env / Claude login / scaffold / stage ---

async fn submit_secure_env_value(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let request_id = args["requestId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "requestId required".to_string()))?
        .to_string();
    let value = args["value"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "value required".to_string()))?
        .to_string();
    let state = state.clone();
    let status = tokio::task::spawn_blocking(move || {
        the_controller_lib::secure_env::submit_secure_env_value_status(
            &state.app,
            &request_id,
            &value,
        )
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task failed: {}", e),
        )
    })?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::String(status)))
}

async fn cancel_secure_env_request(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let request_id = args["requestId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "requestId required".to_string()))?;
    the_controller_lib::secure_env::cancel_secure_env_request(&state.app, request_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

async fn start_claude_login(
    AxumState(state): AxumState<Arc<ServerState>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let session_id = commands::start_claude_login_impl(&state.app)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::String(session_id)))
}

async fn stop_claude_login(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let session_id = args["sessionId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "sessionId required".to_string()))?
        .to_string();
    commands::stop_claude_login_impl(&state.app, session_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

async fn scaffold_project(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let name = args["name"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "name required".to_string()))?
        .to_string();
    let project = commands::scaffold_project_impl(&state.app, name)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(project).unwrap()))
}

async fn stage_session(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "projectId required".to_string()))?;
    let session_id = args["sessionId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "sessionId required".to_string()))?;
    let project_uuid =
        uuid::Uuid::parse_str(project_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let session_uuid =
        uuid::Uuid::parse_str(session_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    commands::stage_session_core(&state.app, project_uuid, session_uuid, true)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

async fn unstage_session(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "projectId required".to_string()))?
        .to_string();
    let session_id = args["sessionId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "sessionId required".to_string()))?
        .to_string();
    commands::unstage_session_impl(&state.app, project_id, session_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

async fn save_screenshot(Json(args): Json<Value>) -> Result<Json<Value>, (StatusCode, String)> {
    use base64::{engine::general_purpose, Engine as _};
    let data_url = args["dataUrl"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "dataUrl required".to_string()))?;
    let b64 = data_url
        .split_once(',')
        .map(|(_, tail)| tail)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "invalid data URL".to_string()))?;
    let bytes = general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid base64: {e}")))?;
    let path = std::env::temp_dir().join("the-controller-screenshot.png");
    std::fs::write(&path, bytes).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("write failed: {e}"),
        )
    })?;
    Ok(Json(Value::String(path.to_string_lossy().to_string())))
}

// --- WebSocket ---

async fn ws_upgrade(
    ws: WebSocketUpgrade,
    AxumState(state): AxumState<Arc<ServerState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state.ws_tx.subscribe()))
}

async fn handle_ws(mut socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    while let Ok(msg) = rx.recv().await {
        if socket.send(Message::Text(msg.into())).await.is_err() {
            break;
        }
    }
}
