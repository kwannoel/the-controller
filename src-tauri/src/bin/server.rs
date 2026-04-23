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
