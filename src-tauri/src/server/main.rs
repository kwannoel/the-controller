use axum::{
    extract::{
        ws::{Message, WebSocket},
        Request, State as AxumState, WebSocketUpgrade,
    },
    http::{
        header::{HOST, ORIGIN},
        HeaderMap, StatusCode,
    },
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use the_controller_lib::{
    architecture, auto_worker, commands, config, deploy, emitter::WsBroadcastEmitter, maintainer,
    models, note_ai_chat, notes, secure_env, session_args, state::AppState, status_socket,
    token_usage, worktree::WorktreeManager,
};

use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

struct ServerState {
    app: Arc<AppState>,
    ws_tx: broadcast::Sender<String>,
}

fn report_startup_error(error: &std::io::Error) {
    tracing::error!("failed to start server: {error}");
    eprintln!("The Controller server failed to start: {error}");
}

fn get_port() -> u16 {
    std::env::var("CONTROLLER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3001)
}

fn get_bind_address() -> String {
    std::env::var("CONTROLLER_BIND").unwrap_or_else(|_| "0.0.0.0".to_string())
}

fn startup_messages(addr: &str, token: Option<&str>) -> (Option<String>, String) {
    match token.filter(|t| !t.is_empty()) {
        Some(_) => {
            (
                Some(format!(
                    "Server listening on http://{} (auth enabled; read CONTROLLER_AUTH_TOKEN from ~/.the-controller/server.env)",
                    addr
                )),
                format!("server listening on http://{} (auth enabled)", addr),
            )
        }
        None => (None, format!("server listening on http://{} (no auth)", addr)),
    }
}

async fn shutdown_signal(state: Arc<ServerState>) {
    let ctrl_c = async { tokio::signal::ctrl_c().await.unwrap() };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .unwrap()
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutting down");
    status_socket::cleanup();

    if let Ok(mut pty_manager) = state.app.pty_manager.lock() {
        let ids = pty_manager.session_ids();
        for id in ids {
            let _ = pty_manager.close_session(id);
        }
    }
}

#[tokio::main]
async fn main() {
    if let Err(error) = run_server().await {
        report_startup_error(&error);
        std::process::exit(1);
    }
}

async fn run_server() -> std::io::Result<()> {
    let base_dir = dirs::home_dir()
        .map(|h| h.join(".the-controller"))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let _log_guard = the_controller_lib::logging::init_backend_logging(&base_dir, true);

    let (emitter, ws_tx) = WsBroadcastEmitter::new();
    let app_state = Arc::new(AppState::new(emitter)?);

    // Start the status socket listener so Claude Code hooks can report session status
    status_socket::start_listener_with_state(app_state.clone());

    let state = Arc::new(ServerState {
        app: app_state,
        ws_tx,
    });

    let dist_dir = std::env::var("CONTROLLER_DIST_DIR").unwrap_or_else(|_| {
        let exe = std::env::current_exe().unwrap_or_default();
        exe.parent()
            .unwrap_or(Path::new("."))
            .join("dist")
            .to_string_lossy()
            .to_string()
    });

    let serve_dir =
        ServeDir::new(&dist_dir).fallback(ServeFile::new(format!("{}/index.html", dist_dir)));
    let auth_enabled = std::env::var("CONTROLLER_AUTH_TOKEN")
        .ok()
        .filter(|t| !t.is_empty());

    let app = Router::new()
        .route("/api/list_projects", post(list_projects))
        .route("/api/check_onboarding", post(check_onboarding))
        .route("/api/restore_sessions", post(restore_sessions))
        .route("/api/connect_session", post(connect_session))
        .route("/api/load_project", post(load_project))
        .route("/api/write_to_pty", post(write_to_pty))
        .route("/api/send_raw_to_pty", post(send_raw_to_pty))
        .route("/api/resize_pty", post(resize_pty))
        .route("/api/close_session", post(close_session))
        .route("/api/create_session", post(create_session))
        .route("/api/create_project", post(create_project))
        .route("/api/delete_project", post(delete_project))
        .route("/api/get_agents_md", post(get_agents_md))
        .route("/api/update_agents_md", post(update_agents_md))
        .route("/api/set_initial_prompt", post(set_initial_prompt))
        .route("/api/check_claude_cli", post(check_claude_cli))
        .route("/api/home_dir", post(home_dir))
        .route("/api/save_onboarding_config", post(save_onboarding_config))
        .route("/api/log_frontend_error", post(log_frontend_error))
        .route("/api/detect_project_type", post(detect_project_type))
        .route("/api/get_deploy_credentials", post(get_deploy_credentials))
        .route(
            "/api/save_deploy_credentials",
            post(save_deploy_credentials),
        )
        .route("/api/is_deploy_provisioned", post(is_deploy_provisioned))
        .route("/api/deploy_project", post(deploy_project))
        .route("/api/list_deployed_services", post(list_deployed_services))
        .route("/api/load_keybindings", post(load_keybindings))
        .route(
            "/api/copy_image_file_to_clipboard",
            post(copy_image_file_to_clipboard),
        )
        .route("/api/capture_app_screenshot", post(capture_app_screenshot))
        .route("/api/start_voice_pipeline", post(start_voice_pipeline))
        .route("/api/stop_voice_pipeline", post(stop_voice_pipeline))
        .route("/api/load_terminal_theme", post(load_terminal_theme))
        .route("/api/list_archived_projects", post(list_archived_projects))
        .route("/api/generate_architecture", post(generate_architecture))
        .route("/api/merge_session_branch", post(merge_session_branch))
        .route("/api/send_note_ai_chat", post(send_note_ai_chat))
        .route("/api/list_notes", post(api_list_notes))
        .route("/api/read_note", post(api_read_note))
        .route("/api/write_note", post(api_write_note))
        .route("/api/create_note", post(api_create_note))
        .route("/api/delete_note", post(api_delete_note))
        .route("/api/rename_note", post(api_rename_note))
        .route("/api/list_folders", post(api_list_folders))
        .route("/api/create_folder", post(api_create_folder))
        .route("/api/rename_folder", post(api_rename_folder))
        .route("/api/delete_folder", post(api_delete_folder))
        .route("/api/commit_notes", post(api_commit_notes))
        // GitHub issues
        .route("/api/list_github_issues", post(list_github_issues))
        .route("/api/list_assigned_issues", post(list_assigned_issues))
        .route("/api/create_github_issue", post(create_github_issue))
        .route("/api/generate_issue_body", post(generate_issue_body))
        .route("/api/post_github_comment", post(post_github_comment))
        .route("/api/add_github_label", post(add_github_label))
        .route("/api/remove_github_label", post(remove_github_label))
        // Maintainer & auto-worker
        .route("/api/configure_maintainer", post(configure_maintainer))
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
        .route("/api/configure_auto_worker", post(configure_auto_worker))
        .route("/api/get_auto_worker_queue", post(get_auto_worker_queue))
        .route("/api/get_worker_reports", post(get_worker_reports))
        // Storage/git operations
        .route("/api/get_session_commits", post(get_session_commits))
        .route("/api/save_session_prompt", post(save_session_prompt))
        .route("/api/list_project_prompts", post(list_project_prompts))
        .route("/api/get_repo_head", post(get_repo_head))
        .route(
            "/api/get_session_token_usage",
            post(get_session_token_usage),
        )
        // Directory listing
        .route("/api/list_directories_at", post(list_directories_at))
        .route("/api/list_root_directories", post(list_root_directories))
        .route("/api/generate_project_names", post(generate_project_names))
        // Scaffold
        .route("/api/scaffold_project", post(scaffold_project))
        // Session management
        .route("/api/stage_session", post(stage_session))
        .route("/api/unstage_session", post(unstage_session))
        .route(
            "/api/submit_secure_env_value",
            post(submit_secure_env_value),
        )
        .route(
            "/api/cancel_secure_env_request",
            post(cancel_secure_env_request),
        )
        // Notes (additional)
        .route("/api/save_note_image", post(api_save_note_image))
        .route(
            "/api/resolve_note_asset_path",
            post(api_resolve_note_asset_path),
        )
        .route("/api/duplicate_note", post(api_duplicate_note))
        // Auth/login
        .route("/api/start_claude_login", post(start_claude_login))
        .route("/api/stop_claude_login", post(stop_claude_login))
        .route("/ws", get(ws_upgrade))
        .fallback_service(serve_dir)
        .layer(middleware::from_fn(auth_middleware))
        .with_state(state.clone());
    let app = if auth_enabled.is_some() {
        app.layer(CorsLayer::permissive())
    } else {
        app
    };

    let port = get_port();
    let bind = get_bind_address();
    let addr = format!("{}:{}", bind, port);
    let (stdout_message, log_message) = startup_messages(&addr, auth_enabled.as_deref());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    if let Some(message) = stdout_message {
        println!("{}", message);
    }
    tracing::info!("{}", log_message);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(state))
        .await
        .map_err(std::io::Error::other)
}

// --- Auth middleware ---

async fn auth_middleware(req: Request, next: Next) -> Result<Response, StatusCode> {
    let path = req.uri().path();
    if !is_api_or_ws_path(path) {
        return Ok(next.run(req).await);
    }

    let token = match std::env::var("CONTROLLER_AUTH_TOKEN") {
        Ok(t) if !t.is_empty() => t,
        _ => {
            if request_origin_allowed(&req) {
                return Ok(next.run(req).await);
            }
            return Err(StatusCode::FORBIDDEN);
        }
    };

    // Check Authorization header or query param
    let authorized = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.strip_prefix("Bearer ").unwrap_or(v) == token)
        .unwrap_or(false)
        || req
            .uri()
            .query()
            .and_then(|query| decoded_query_param(query, "token"))
            .map(|candidate| candidate == token)
            .unwrap_or(false);

    if authorized {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

fn is_api_or_ws_path(path: &str) -> bool {
    path.starts_with("/api/") || path == "/ws"
}

fn decoded_query_param(query: &str, key: &str) -> Option<String> {
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix(&format!("{key}=")))
        .and_then(percent_decode)
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let hi = decode_hex(bytes[index + 1])?;
                let lo = decode_hex(bytes[index + 2])?;
                decoded.push((hi << 4) | lo);
                index += 3;
            }
            b'%' => return None,
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }

    String::from_utf8(decoded).ok()
}

fn decode_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn request_origin_allowed(req: &Request) -> bool {
    match request_origin(req.headers()) {
        None => true,
        Some("null") => false,
        Some(origin) => expected_request_origin(req.headers()).as_deref() == Some(origin),
    }
}

fn request_origin(headers: &HeaderMap) -> Option<&str> {
    headers.get(ORIGIN).and_then(|value| value.to_str().ok())
}

fn expected_request_origin(headers: &HeaderMap) -> Option<String> {
    let host = headers.get(HOST)?.to_str().ok()?;
    let proto = forwarded_proto(headers).unwrap_or("http");
    Some(format!("{proto}://{host}"))
}

fn forwarded_proto(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
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
            tracing::error!(
                "failed to migrate worktrees for project '{}': {}",
                project.name,
                e
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
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing name".to_string()))?
        .to_string();
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing repoPath".to_string()))?
        .to_string();

    commands::validate_project_name(&name).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let path = std::path::Path::new(&repo_path);
    if !path.is_dir() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("repo_path is not a directory: {}", repo_path),
        ));
    }

    // Validate it's a git repo
    let git_dir = path.join(".git");
    if !git_dir.exists() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("not a git repository: {}", repo_path),
        ));
    }

    let storage = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Return existing project if one with the same repo_path exists
    if let Ok(inventory) = storage.list_projects() {
        let existing = inventory.projects;
        if let Some(project) = existing.iter().find(|p| p.repo_path == repo_path) {
            return Ok(Json(serde_json::to_value(project.clone()).unwrap()));
        }
        // Reject duplicate project names when creating new
        if existing.iter().any(|p| p.name == name) {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("A project named '{}' already exists", name),
            ));
        }
    }

    let project = models::Project {
        id: uuid::Uuid::new_v4(),
        name: name.clone(),
        repo_path: repo_path.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        archived: false,
        maintainer: models::MaintainerConfig::default(),
        auto_worker: models::AutoWorkerConfig::default(),
        prompts: vec![],
        sessions: vec![],
        staged_session: None,
    };

    storage
        .save_project(&project)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Only create default agents.md if repo doesn't have one
    let repo_agents = path.join("agents.md");
    if !repo_agents.exists() {
        storage
            .save_agents_md(project.id, &commands::render_agents_md(&project.name))
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    // If repo has agents.md but no CLAUDE.md, create symlink
    commands::ensure_claude_md_symlink(path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::to_value(project).unwrap()))
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
    let project_id = args["projectId"].as_str().unwrap_or_default();
    let session_id = args["sessionId"].as_str().unwrap_or_default();
    let delete_worktree = args["deleteWorktree"].as_bool().unwrap_or(false);
    let project_uuid =
        uuid::Uuid::parse_str(project_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let session_uuid =
        uuid::Uuid::parse_str(session_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    // Close PTY / kill broker session
    {
        let mut pty_manager = state
            .app
            .pty_manager
            .lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let _ = pty_manager.close_session(session_uuid);
    }

    // Remove session from project
    let (repo_path, session) = {
        let storage = state
            .app
            .storage
            .lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let mut project = storage
            .load_project(project_uuid)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let session = project
            .sessions
            .iter()
            .find(|s| s.id == session_uuid)
            .cloned();
        project.sessions.retain(|s| s.id != session_uuid);
        storage
            .save_project(&project)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        (project.repo_path.clone(), session)
    };

    // Optionally delete worktree
    if delete_worktree {
        if let Some(session) = session {
            if let (Some(wt_path), Some(branch)) = (session.worktree_path, session.worktree_branch)
            {
                let rp = repo_path;
                let _ = tokio::task::spawn_blocking(move || {
                    the_controller_lib::worktree::WorktreeManager::remove_worktree(
                        &wt_path, &rp, &branch,
                    )
                })
                .await;
            }
        }
    }

    Ok(Json(Value::Null))
}

async fn create_session(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"].as_str().unwrap_or_default();
    let kind = args["kind"].as_str().unwrap_or("claude").to_string();
    let background = args["background"].as_bool().unwrap_or(false);
    let initial_prompt = args["initialPrompt"].as_str().map(|s| s.to_string());
    let github_issue: Option<models::GithubIssue> =
        serde_json::from_value(args["githubIssue"].clone()).ok();

    let project_uuid =
        uuid::Uuid::parse_str(project_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let session_id = uuid::Uuid::new_v4();

    // Load the project and generate session label
    let (repo_path, label, base_dir, project_name) = {
        let storage = state
            .app
            .storage
            .lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let project = storage
            .load_project(project_uuid)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let label = commands::next_session_label(&project.sessions);
        (
            project.repo_path.clone(),
            label,
            storage.base_dir(),
            project.name.clone(),
        )
    };

    // Create worktree under ~/.the-controller/worktrees/{project_name}/{label}/
    let worktree_dir = base_dir.join("worktrees").join(&project_name).join(&label);

    let repo_path_clone = repo_path.clone();
    let label_clone = label.clone();
    let (session_dir, wt_path, wt_branch) = tokio::task::spawn_blocking(move || {
        match WorktreeManager::create_worktree(&repo_path_clone, &label_clone, &worktree_dir) {
            Ok(worktree_path) => {
                let wt_str = worktree_path
                    .to_str()
                    .ok_or_else(|| "worktree path is not valid UTF-8".to_string())?
                    .to_string();
                Ok((wt_str.clone(), Some(wt_str), Some(label_clone)))
            }
            Err(e) if e == "unborn_branch" => Ok((repo_path_clone, None, None)),
            Err(e) => Err(e),
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

    // Build initial prompt: explicit prompt takes priority, then GitHub issue context
    let initial_prompt = initial_prompt.or_else(|| {
        github_issue.as_ref().map(|issue| {
            session_args::build_issue_prompt(issue.number, &issue.title, &issue.url, background)
        })
    });

    let session_config = models::SessionConfig {
        id: session_id,
        label: label.clone(),
        worktree_path: wt_path.clone(),
        worktree_branch: wt_branch.clone(),
        archived: false,
        kind: kind.clone(),
        github_issue,
        initial_prompt: initial_prompt.clone(),
        done_commits: vec![],
        auto_worker_session: false,
    };

    // Save session config to project
    {
        let storage = state
            .app
            .storage
            .lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let mut project = storage
            .load_project(project_uuid)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        project.sessions.push(session_config);
        storage
            .save_project(&project)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    // Spawn PTY session
    let pty_manager = state.app.pty_manager.clone();
    let emitter = state.app.emitter.clone();
    let spawn_result = tokio::task::spawn_blocking(move || {
        let mut mgr = pty_manager.lock().map_err(|e| e.to_string())?;
        mgr.spawn_session(
            session_id,
            &session_dir,
            &kind,
            emitter,
            false,
            initial_prompt.as_deref(),
            24,
            80,
        )
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task failed: {}", e),
        )
    })?;

    if let Err(spawn_err) = spawn_result {
        // Rollback: remove session from project
        if let Ok(storage) = state.app.storage.lock() {
            if let Ok(mut project) = storage.load_project(project_uuid) {
                project.sessions.retain(|s| s.id != session_id);
                let _ = storage.save_project(&project);
            }
        }
        // Cleanup worktree
        if let (Some(ref wt_path), Some(ref wt_branch)) = (wt_path, wt_branch) {
            let _ = WorktreeManager::remove_worktree(wt_path, &repo_path, wt_branch);
        }
        return Err((StatusCode::INTERNAL_SERVER_ERROR, spawn_err));
    }

    Ok(Json(Value::String(session_id.to_string())))
}

async fn generate_architecture(
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing repoPath".to_string()))?
        .to_string();

    let result = tokio::task::spawn_blocking(move || {
        architecture::generate_architecture_blocking(std::path::Path::new(&repo_path))
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task failed: {}", e),
        )
    })?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::to_value(result).unwrap()))
}

async fn create_project(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let name = args["name"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing name".to_string()))?
        .to_string();
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing repoPath".to_string()))?
        .to_string();

    commands::validate_project_name(&name).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let path = std::path::Path::new(&repo_path);
    if !path.is_dir() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("repo_path is not a directory: {}", repo_path),
        ));
    }

    let storage = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Reject duplicate project names
    if let Ok(inventory) = storage.list_projects() {
        if inventory.projects.iter().any(|p| p.name == name) {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("A project named '{}' already exists", name),
            ));
        }
    }

    let project = models::Project {
        id: uuid::Uuid::new_v4(),
        name: name.clone(),
        repo_path: repo_path.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        archived: false,
        maintainer: models::MaintainerConfig::default(),
        auto_worker: models::AutoWorkerConfig::default(),
        prompts: vec![],
        sessions: vec![],
        staged_session: None,
    };

    storage
        .save_project(&project)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // If repo doesn't have agents.md, create default one in config dir
    let repo_agents = path.join("agents.md");
    if !repo_agents.exists() {
        storage
            .save_agents_md(project.id, &commands::render_agents_md(&project.name))
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    // If repo has agents.md but no CLAUDE.md, create symlink
    commands::ensure_claude_md_symlink(path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::to_value(project).unwrap()))
}

async fn delete_project(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"].as_str().unwrap_or_default();
    let delete_repo = args["deleteRepo"].as_bool().unwrap_or(false);
    let id =
        uuid::Uuid::parse_str(project_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let storage = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let project = storage
        .load_project(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Close all PTY sessions and clean up worktrees
    {
        let mut pty_manager = state
            .app
            .pty_manager
            .lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        for session in &project.sessions {
            let _ = pty_manager.close_session(session.id);
            if let (Some(wt_path), Some(branch)) =
                (&session.worktree_path, &session.worktree_branch)
            {
                let _ = WorktreeManager::remove_worktree(wt_path, &project.repo_path, branch);
            }
        }
    }

    // Delete project metadata
    storage
        .delete_project_dir(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Optionally delete the repo directory
    if delete_repo && std::path::Path::new(&project.repo_path).exists() {
        std::fs::remove_dir_all(&project.repo_path).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to delete repo: {}", e),
            )
        })?;
    }

    Ok(Json(Value::Null))
}

async fn get_agents_md(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"].as_str().unwrap_or_default();
    let id =
        uuid::Uuid::parse_str(project_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let storage = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let project = storage
        .load_project(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let content = storage
        .get_agents_md(&project)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(Value::String(content)))
}

async fn update_agents_md(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"].as_str().unwrap_or_default();
    let content = args["content"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing content".to_string()))?;
    let id =
        uuid::Uuid::parse_str(project_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let storage = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    storage
        .save_agents_md(id, content)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(Value::Null))
}

async fn set_initial_prompt(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"].as_str().unwrap_or_default();
    let session_id = args["sessionId"].as_str().unwrap_or_default();
    let prompt = args["prompt"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing prompt".to_string()))?
        .to_string();

    let project_uuid =
        uuid::Uuid::parse_str(project_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let session_uuid =
        uuid::Uuid::parse_str(session_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let storage = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut project = storage
        .load_project(project_uuid)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if let Some(session) = project.sessions.iter_mut().find(|s| s.id == session_uuid) {
        if session.initial_prompt.is_none() {
            session.initial_prompt = Some(prompt);
            storage
                .save_project(&project)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
    }

    Ok(Json(Value::Null))
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

async fn home_dir() -> Result<Json<Value>, (StatusCode, String)> {
    let path = dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not determine home directory".to_string(),
            )
        })?;
    Ok(Json(Value::String(path)))
}

async fn save_onboarding_config(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let projects_root = args["projectsRoot"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing projectsRoot".to_string()))?
        .to_string();
    let default_provider: config::ConfigDefaultProvider =
        serde_json::from_value(args["defaultProvider"].clone()).unwrap_or_default();

    let path = std::path::Path::new(&projects_root);
    if !path.is_dir() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "projects_root is not an existing directory: {}",
                projects_root
            ),
        ));
    }

    let storage = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let base_dir = storage.base_dir();

    // Preserve existing log_level to avoid clobbering it
    let existing_log_level = config::load_config(&base_dir)
        .map(|c| c.log_level)
        .unwrap_or_else(|| "info".to_string());

    let cfg = config::Config {
        projects_root,
        default_provider,
        log_level: existing_log_level,
    };
    config::save_config(&base_dir, &cfg)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(Value::Null))
}

async fn log_frontend_error(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    use std::io::Write;
    let message = args["message"].as_str().unwrap_or_default();
    let sanitized = message.replace('\n', "\\n").replace('\r', "\\r");
    let timestamp = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%:z");
    let line = format!("{} ERROR [frontend] {}\n", timestamp, sanitized);

    if let Ok(mut guard) = state.app.frontend_log.lock() {
        if let Some(ref mut file) = *guard {
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
    }

    tracing::error!(target: "frontend", "{}", sanitized);
    Ok(Json(Value::Null))
}

async fn detect_project_type(Json(args): Json<Value>) -> Result<Json<Value>, (StatusCode, String)> {
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing repoPath".to_string()))?
        .to_string();
    let result = deploy::commands::detect_project_type(repo_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(result).unwrap()))
}

async fn get_deploy_credentials() -> Result<Json<Value>, (StatusCode, String)> {
    let result = deploy::commands::get_deploy_credentials()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(result).unwrap()))
}

async fn save_deploy_credentials(
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let credentials: deploy::credentials::DeployCredentials =
        serde_json::from_value(args["credentials"].clone())
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    deploy::commands::save_deploy_credentials(credentials)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

async fn is_deploy_provisioned() -> Result<Json<Value>, (StatusCode, String)> {
    let provisioned = deploy::commands::is_deploy_provisioned()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Bool(provisioned)))
}

async fn deploy_project(Json(args): Json<Value>) -> Result<Json<Value>, (StatusCode, String)> {
    let request: deploy::commands::DeployRequest = serde_json::from_value(args["request"].clone())
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let result = deploy::commands::deploy_project(request)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(result).unwrap()))
}

async fn list_deployed_services() -> Result<Json<Value>, (StatusCode, String)> {
    let services = deploy::commands::list_deployed_services()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Array(services)))
}

async fn load_keybindings(
    AxumState(state): AxumState<Arc<ServerState>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let base_dir = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    let result = the_controller_lib::keybindings::load_keybindings(&base_dir);
    Ok(Json(serde_json::to_value(result).unwrap()))
}

// --- Desktop-only stubs (return NOT_IMPLEMENTED gracefully) ---

async fn copy_image_file_to_clipboard() -> Result<Json<Value>, (StatusCode, String)> {
    Err((
        StatusCode::NOT_IMPLEMENTED,
        "copy_image_file_to_clipboard is not available in server mode".to_string(),
    ))
}

async fn capture_app_screenshot() -> Result<Json<Value>, (StatusCode, String)> {
    Err((
        StatusCode::NOT_IMPLEMENTED,
        "capture_app_screenshot is not available in server mode".to_string(),
    ))
}

async fn start_voice_pipeline() -> Result<Json<Value>, (StatusCode, String)> {
    Err((
        StatusCode::NOT_IMPLEMENTED,
        "start_voice_pipeline is not available in server mode".to_string(),
    ))
}

async fn stop_voice_pipeline() -> Result<Json<Value>, (StatusCode, String)> {
    Err((
        StatusCode::NOT_IMPLEMENTED,
        "stop_voice_pipeline is not available in server mode".to_string(),
    ))
}

async fn load_terminal_theme(
    AxumState(state): AxumState<Arc<ServerState>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let base_dir = {
        let storage = state
            .app
            .storage
            .lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        storage.base_dir()
    };
    let theme = tokio::task::spawn_blocking(move || {
        the_controller_lib::terminal_theme::load_terminal_theme(&base_dir)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task failed: {}", e),
        )
    })?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::to_value(theme).unwrap()))
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

    const OVERALL_TIMEOUT_SECS: u64 = 600; // 10 minutes

    let merge_result = tokio::time::timeout(
        std::time::Duration::from_secs(OVERALL_TIMEOUT_SECS),
        do_merge_with_retries(
            &state,
            &repo_path,
            &worktree_path,
            &branch_name,
            session_uuid,
        ),
    )
    .await;

    match merge_result {
        Ok(inner) => inner,
        Err(_) => Err((
            StatusCode::GATEWAY_TIMEOUT,
            format!("Merge timed out after {} seconds", OVERALL_TIMEOUT_SECS),
        )),
    }
}

async fn do_merge_with_retries(
    state: &Arc<ServerState>,
    repo_path: &str,
    worktree_path: &str,
    branch_name: &str,
    session_uuid: uuid::Uuid,
) -> Result<Json<Value>, (StatusCode, String)> {
    use the_controller_lib::models::MergeResponse;
    use the_controller_lib::worktree::{MergeResult, WorktreeManager};

    const MAX_RETRIES: u32 = 5;
    const POLL_INTERVAL_SECS: u64 = 3;

    for attempt in 0..MAX_RETRIES {
        let rp = repo_path.to_string();
        let wt = worktree_path.to_string();
        let br = branch_name.to_string();

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

                let wt_poll = worktree_path.to_string();
                let max_polls = 200; // 600s / 3s
                let mut poll_count = 0;
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
                    poll_count += 1;
                    if poll_count >= max_polls {
                        break;
                    }
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

async fn send_note_ai_chat(Json(args): Json<Value>) -> Result<Json<Value>, (StatusCode, String)> {
    let note_content = args["noteContent"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing noteContent".to_string()))?
        .to_string();
    let selected_text = args["selectedText"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing selectedText".to_string()))?
        .to_string();
    let prompt = args["prompt"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing prompt".to_string()))?
        .to_string();

    let conversation_history: Vec<note_ai_chat::NoteAiChatMessage> =
        serde_json::from_value(args["conversationHistory"].clone()).unwrap_or_default();

    let response = note_ai_chat::send_note_ai_message(
        std::env::temp_dir().to_string_lossy().to_string(),
        note_content,
        selected_text,
        conversation_history,
        prompt,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::to_value(response).unwrap()))
}
// --- Notes ---

fn server_try_commit(state: &Arc<ServerState>, message: &str) {
    if let Ok(storage) = state.app.storage.lock() {
        let base_dir = storage.base_dir();
        if let Err(e) = notes::commit_notes(&base_dir, message) {
            tracing::error!("notes git commit failed: {}", e);
        }
    }
}

async fn api_list_notes(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let folder = args["folder"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing folder".to_string()))?
        .to_string();
    let base_dir = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    let entries = notes::list_notes(&base_dir, &folder)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::to_value(entries).unwrap()))
}

async fn api_read_note(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let folder = args["folder"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing folder".to_string()))?
        .to_string();
    let filename = args["filename"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing filename".to_string()))?
        .to_string();
    let base_dir = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    let content = notes::read_note(&base_dir, &folder, &filename)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::to_value(content).unwrap()))
}

async fn api_write_note(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let folder = args["folder"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing folder".to_string()))?
        .to_string();
    let filename = args["filename"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing filename".to_string()))?
        .to_string();
    let content = args["content"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing content".to_string()))?
        .to_string();
    let base_dir = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    notes::write_note(&base_dir, &folder, &filename, &content)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(Value::Null))
}

async fn api_create_note(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let folder = args["folder"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing folder".to_string()))?
        .to_string();
    let title = args["title"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing title".to_string()))?
        .to_string();
    let base_dir = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    let filename = notes::create_note(&base_dir, &folder, &title)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    server_try_commit(&state, &format!("create {}/{}", folder, filename));
    Ok(Json(serde_json::to_value(filename).unwrap()))
}

async fn api_delete_note(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let folder = args["folder"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing folder".to_string()))?
        .to_string();
    let filename = args["filename"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing filename".to_string()))?
        .to_string();
    let base_dir = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    notes::delete_note(&base_dir, &folder, &filename)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    server_try_commit(&state, &format!("delete {}/{}", folder, filename));
    Ok(Json(Value::Null))
}

async fn api_rename_note(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let folder = args["folder"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing folder".to_string()))?
        .to_string();
    let old_name = args["oldName"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing oldName".to_string()))?
        .to_string();
    let new_name = args["newName"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing newName".to_string()))?
        .to_string();
    let base_dir = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    let filename = notes::rename_note(&base_dir, &folder, &old_name, &new_name)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    server_try_commit(
        &state,
        &format!("rename {}/{} → {}", folder, old_name, filename),
    );
    Ok(Json(serde_json::to_value(filename).unwrap()))
}

async fn api_list_folders(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(_args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let base_dir = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    let folders = notes::list_folders(&base_dir)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::to_value(folders).unwrap()))
}

async fn api_create_folder(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let name = args["name"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing name".to_string()))?
        .to_string();
    let base_dir = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    notes::create_folder(&base_dir, &name)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    server_try_commit(&state, &format!("create folder {}", name));
    Ok(Json(Value::Null))
}

async fn api_rename_folder(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let old_name = args["oldName"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing oldName".to_string()))?
        .to_string();
    let new_name = args["newName"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing newName".to_string()))?
        .to_string();
    let base_dir = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    notes::rename_folder(&base_dir, &old_name, &new_name)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    server_try_commit(
        &state,
        &format!("rename folder {} → {}", old_name, new_name),
    );
    Ok(Json(Value::Null))
}

async fn api_delete_folder(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let name = args["name"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing name".to_string()))?
        .to_string();
    let force = args["force"].as_bool().unwrap_or(false);
    let base_dir = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    notes::delete_folder(&base_dir, &name, force)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    server_try_commit(&state, &format!("delete folder {}", name));
    Ok(Json(Value::Null))
}

async fn api_commit_notes(
    AxumState(state): AxumState<Arc<ServerState>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let base_dir = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    let committed = notes::commit_notes(&base_dir, "update notes")
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::to_value(committed).unwrap()))
}

// --- GitHub Issues ---

async fn list_github_issues(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing repoPath".to_string()))?
        .to_string();

    // Check cache
    let cache_result = {
        let cache = state
            .app
            .issue_cache
            .lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        match cache.get(&repo_path) {
            Some(entry) if entry.is_fresh() => {
                return Ok(Json(serde_json::to_value(&entry.issues).unwrap()))
            }
            Some(entry) => Some(entry.issues.clone()),
            None => None,
        }
    };

    if let Some(stale_issues) = cache_result {
        let cache_arc = state.app.issue_cache.clone();
        let repo_bg = repo_path.clone();
        tokio::spawn(async move {
            if let Ok(fresh) = fetch_github_issues_async(&repo_bg).await {
                if let Ok(mut cache) = cache_arc.lock() {
                    cache.insert(repo_bg, fresh);
                }
            }
        });
        return Ok(Json(serde_json::to_value(stale_issues).unwrap()));
    }

    let issues = fetch_github_issues_async(&repo_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    {
        let mut cache = state
            .app
            .issue_cache
            .lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        cache.insert(repo_path, issues.clone());
    }
    Ok(Json(serde_json::to_value(issues).unwrap()))
}

async fn extract_github_repo_async(repo_path: &str) -> Result<String, String> {
    let rp = repo_path.to_string();
    tokio::task::spawn_blocking(move || {
        let repo =
            git2::Repository::discover(&rp).map_err(|e| format!("Failed to open repo: {}", e))?;
        let remote = repo
            .find_remote("origin")
            .map_err(|_| "No 'origin' remote found".to_string())?;
        let url = remote
            .url()
            .ok_or_else(|| "Origin remote URL is not valid UTF-8".to_string())?;
        parse_github_nwo(url)
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
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

async fn fetch_github_issues_async(repo_path: &str) -> Result<Vec<models::GithubIssue>, String> {
    let nwo = extract_github_repo_async(repo_path).await?;
    let output = tokio::process::Command::new("gh")
        .args([
            "issue",
            "list",
            "--repo",
            &nwo,
            "--json",
            "number,title,url,body,labels",
            "--limit",
            "50",
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to run gh: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh issue list failed: {}", stderr));
    }
    serde_json::from_slice(&output.stdout).map_err(|e| format!("Failed to parse gh output: {}", e))
}

async fn list_assigned_issues(
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing repoPath".to_string()))?
        .to_string();
    let nwo = extract_github_repo_async(&repo_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let output = tokio::process::Command::new("gh")
        .args([
            "issue",
            "list",
            "--repo",
            &nwo,
            "--json",
            "number,title,url,assignees,updatedAt,labels",
            "--limit",
            "100",
        ])
        .output()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to run gh: {}", e),
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("gh issue list failed: {}", stderr),
        ));
    }
    let all: Vec<models::AssignedIssue> = serde_json::from_slice(&output.stdout).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to parse gh output: {}", e),
        )
    })?;
    let assigned: Vec<_> = all
        .into_iter()
        .filter(|i| !i.assignees.is_empty())
        .collect();
    Ok(Json(serde_json::to_value(assigned).unwrap()))
}

async fn create_github_issue(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing repoPath".to_string()))?
        .to_string();
    let title = args["title"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing title".to_string()))?
        .to_string();
    let body = args["body"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing body".to_string()))?
        .to_string();
    let nwo = extract_github_repo_async(&repo_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let output = tokio::process::Command::new("gh")
        .args([
            "issue", "create", "--repo", &nwo, "--title", &title, "--body", &body,
        ])
        .output()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to run gh: {}", e),
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("gh issue create failed: {}", stderr),
        ));
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let number = url
        .rsplit('/')
        .next()
        .and_then(|s| s.parse::<u64>().ok())
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not parse issue number".to_string(),
            )
        })?;
    let issue = models::GithubIssue {
        number,
        title,
        url,
        body: Some(body),
        labels: vec![],
    };
    if let Ok(mut cache) = state.app.issue_cache.lock() {
        cache.add_issue(&repo_path, issue.clone());
    }
    Ok(Json(serde_json::to_value(issue).unwrap()))
}

async fn generate_issue_body(Json(args): Json<Value>) -> Result<Json<Value>, (StatusCode, String)> {
    let title = args["title"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing title".to_string()))?
        .to_string();
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
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to run claude: {}", e),
            )
        })?;
    let body = if output.status.success() {
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    } else {
        String::new()
    };
    Ok(Json(Value::String(body)))
}

async fn post_github_comment(Json(args): Json<Value>) -> Result<Json<Value>, (StatusCode, String)> {
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing repoPath".to_string()))?
        .to_string();
    let issue_number = args["issueNumber"]
        .as_u64()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing issueNumber".to_string()))?;
    let body = args["body"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing body".to_string()))?
        .to_string();
    let nwo = extract_github_repo_async(&repo_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let output = tokio::process::Command::new("gh")
        .args([
            "issue",
            "comment",
            &issue_number.to_string(),
            "--repo",
            &nwo,
            "--body",
            &body,
        ])
        .output()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to run gh: {}", e),
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("gh issue comment failed: {}", stderr),
        ));
    }
    Ok(Json(Value::Null))
}

async fn add_github_label(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing repoPath".to_string()))?
        .to_string();
    let issue_number = args["issueNumber"]
        .as_u64()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing issueNumber".to_string()))?;
    let label = args["label"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing label".to_string()))?
        .to_string();
    let description = args["description"].as_str().map(|s| s.to_string());
    let color = args["color"].as_str().map(|s| s.to_string());
    let nwo = extract_github_repo_async(&repo_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let desc = description
        .as_deref()
        .unwrap_or("Issue is being worked on in a session");
    let col = color.as_deref().unwrap_or("F9E2AF");
    let _ = tokio::process::Command::new("gh")
        .args([
            "label",
            "create",
            &label,
            "--repo",
            &nwo,
            "--description",
            desc,
            "--color",
            col,
        ])
        .output()
        .await;
    let output = tokio::process::Command::new("gh")
        .args([
            "issue",
            "edit",
            &issue_number.to_string(),
            "--repo",
            &nwo,
            "--add-label",
            &label,
        ])
        .output()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to run gh: {}", e),
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("gh issue edit failed: {}", stderr),
        ));
    }
    if let Ok(mut cache) = state.app.issue_cache.lock() {
        cache.add_label(&repo_path, issue_number, &label);
    }
    Ok(Json(Value::Null))
}

async fn remove_github_label(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing repoPath".to_string()))?
        .to_string();
    let issue_number = args["issueNumber"]
        .as_u64()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing issueNumber".to_string()))?;
    let label = args["label"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing label".to_string()))?
        .to_string();
    let nwo = extract_github_repo_async(&repo_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let output = tokio::process::Command::new("gh")
        .args([
            "issue",
            "edit",
            &issue_number.to_string(),
            "--repo",
            &nwo,
            "--remove-label",
            &label,
        ])
        .output()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to run gh: {}", e),
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("gh issue edit failed: {}", stderr),
        ));
    }
    if let Ok(mut cache) = state.app.issue_cache.lock() {
        cache.remove_label(&repo_path, issue_number, &label);
    }
    Ok(Json(Value::Null))
}

// --- Maintainer & Auto-Worker ---

async fn configure_maintainer(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing projectId".to_string()))?;
    let enabled = args["enabled"].as_bool().unwrap_or(false);
    let interval_minutes = args["intervalMinutes"].as_u64().unwrap_or(30);
    let github_repo = args["githubRepo"].as_str().map(|s| s.to_string());
    commands::validate_maintainer_interval(interval_minutes)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let project_uuid =
        uuid::Uuid::parse_str(project_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let storage = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut project = storage
        .load_project(project_uuid)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    project.maintainer.enabled = enabled;
    project.maintainer.interval_minutes = interval_minutes;
    project.maintainer.github_repo = github_repo;
    storage
        .save_project(&project)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(Value::Null))
}

async fn get_maintainer_status(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing projectId".to_string()))?;
    let project_uuid =
        uuid::Uuid::parse_str(project_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let storage = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let log = storage
        .latest_maintainer_run_log(project_uuid)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::to_value(log).unwrap()))
}

async fn get_maintainer_history(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing projectId".to_string()))?;
    let project_uuid =
        uuid::Uuid::parse_str(project_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let storage = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let logs = storage
        .maintainer_run_log_history(project_uuid, 20)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::to_value(logs).unwrap()))
}

async fn trigger_maintainer_check(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing projectId".to_string()))?;
    let project_uuid =
        uuid::Uuid::parse_str(project_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let (repo_path, github_repo) = {
        let storage = state
            .app
            .storage
            .lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let project = storage
            .load_project(project_uuid)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        (
            project.repo_path.clone(),
            project.maintainer.github_repo.clone(),
        )
    };
    let _ = state
        .app
        .emitter
        .emit(&format!("maintainer-status:{}", project_uuid), "running");
    let log = tokio::task::spawn_blocking(move || {
        maintainer::run_maintainer_check(&repo_path, project_uuid, github_repo.as_deref())
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task failed: {}", e),
        )
    })?
    .map_err(|e| {
        let _ = state
            .app
            .emitter
            .emit(&format!("maintainer-status:{}", project_uuid), "error");
        let _ = state
            .app
            .emitter
            .emit(&format!("maintainer-error:{}", project_uuid), &e);
        (StatusCode::INTERNAL_SERVER_ERROR, e)
    })?;
    {
        let storage = state
            .app
            .storage
            .lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        storage
            .save_maintainer_run_log(&log)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    let _ = state
        .app
        .emitter
        .emit(&format!("maintainer-status:{}", project_uuid), "idle");
    Ok(Json(serde_json::to_value(log).unwrap()))
}

async fn clear_maintainer_reports(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing projectId".to_string()))?;
    let project_uuid =
        uuid::Uuid::parse_str(project_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let storage = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    storage
        .clear_maintainer_run_logs(project_uuid)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let _ = state
        .app
        .emitter
        .emit(&format!("maintainer-status:{}", project_uuid), "idle");
    Ok(Json(Value::Null))
}

async fn get_maintainer_issues(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing projectId".to_string()))?;
    let project_uuid =
        uuid::Uuid::parse_str(project_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let (repo_path, github_repo) = {
        let storage = state
            .app
            .storage
            .lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let project = storage
            .load_project(project_uuid)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        (
            project.repo_path.clone(),
            project.maintainer.github_repo.clone(),
        )
    };
    let nwo = match github_repo {
        Some(ref repo) if !repo.is_empty() => repo.clone(),
        _ => extract_github_repo_async(&repo_path)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?,
    };
    let output = tokio::process::Command::new("gh")
        .args([
            "issue",
            "list",
            "--repo",
            &nwo,
            "--label",
            "filed-by-maintainer",
            "--state",
            "all",
            "--json",
            "number,title,state,url,labels,createdAt,closedAt",
            "--limit",
            "100",
        ])
        .output()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to run gh: {}", e),
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("gh issue list failed: {}", stderr),
        ));
    }
    let issues: Vec<models::MaintainerIssue> =
        serde_json::from_slice(&output.stdout).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to parse gh output: {}", e),
            )
        })?;
    Ok(Json(serde_json::to_value(issues).unwrap()))
}

async fn get_maintainer_issue_detail(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing projectId".to_string()))?;
    let issue_number = args["issueNumber"]
        .as_u64()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing issueNumber".to_string()))?
        as u32;
    let project_uuid =
        uuid::Uuid::parse_str(project_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let (repo_path, github_repo) = {
        let storage = state
            .app
            .storage
            .lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let project = storage
            .load_project(project_uuid)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        (
            project.repo_path.clone(),
            project.maintainer.github_repo.clone(),
        )
    };
    let nwo = match github_repo {
        Some(ref repo) if !repo.is_empty() => repo.clone(),
        _ => extract_github_repo_async(&repo_path)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?,
    };
    let output = tokio::process::Command::new("gh")
        .args([
            "issue",
            "view",
            &issue_number.to_string(),
            "--repo",
            &nwo,
            "--json",
            "number,title,state,body,url,labels,createdAt,closedAt",
        ])
        .output()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to run gh: {}", e),
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("gh issue view failed: {}", stderr),
        ));
    }
    let detail: models::MaintainerIssueDetail =
        serde_json::from_slice(&output.stdout).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to parse gh output: {}", e),
            )
        })?;
    Ok(Json(serde_json::to_value(detail).unwrap()))
}

async fn configure_auto_worker(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing projectId".to_string()))?;
    let enabled = args["enabled"].as_bool().unwrap_or(false);
    let project_uuid =
        uuid::Uuid::parse_str(project_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let storage = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut project = storage
        .load_project(project_uuid)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    project.auto_worker.enabled = enabled;
    storage
        .save_project(&project)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(Value::Null))
}

async fn get_auto_worker_queue(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing projectId".to_string()))?;
    let project_uuid =
        uuid::Uuid::parse_str(project_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let project = {
        let storage = state
            .app
            .storage
            .lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        storage
            .load_project(project_uuid)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };
    let active_issue = project
        .sessions
        .iter()
        .find(|s| s.auto_worker_session)
        .and_then(|s| s.github_issue.clone());
    let issues = fetch_github_issues_async(&project.repo_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    // Update cache
    if let Ok(mut cache) = state.app.issue_cache.lock() {
        cache.insert(project.repo_path.clone(), issues.clone());
    }
    let active_number = active_issue.as_ref().map(|i| i.number);
    let mut queue = Vec::new();
    if let Some(issue) = active_issue {
        queue.push(models::AutoWorkerQueueIssue {
            number: issue.number,
            title: issue.title,
            url: issue.url,
            body: issue.body,
            labels: issue.labels.into_iter().map(|l| l.name).collect(),
            is_active: true,
        });
    }
    queue.extend(
        issues
            .into_iter()
            .filter(auto_worker::is_eligible)
            .filter(|i| Some(i.number) != active_number)
            .map(|i| models::AutoWorkerQueueIssue {
                number: i.number,
                title: i.title,
                url: i.url,
                body: i.body,
                labels: i.labels.into_iter().map(|l| l.name).collect(),
                is_active: false,
            }),
    );
    Ok(Json(serde_json::to_value(queue).unwrap()))
}

async fn get_worker_reports(Json(args): Json<Value>) -> Result<Json<Value>, (StatusCode, String)> {
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing repoPath".to_string()))?
        .to_string();
    let nwo = extract_github_repo_async(&repo_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let output = tokio::process::Command::new("gh")
        .args([
            "issue",
            "list",
            "--repo",
            &nwo,
            "--label",
            "assigned-to-auto-worker",
            "--state",
            "all",
            "--json",
            "number,title,state,comments,updatedAt",
            "--limit",
            "50",
        ])
        .output()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to run gh: {}", e),
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("gh issue list failed: {}", stderr),
        ));
    }
    let raw: Vec<Value> = serde_json::from_slice(&output.stdout).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to parse gh output: {}", e),
        )
    })?;
    let reports: Vec<Value> = raw
        .into_iter()
        .filter_map(|issue| {
            if issue["state"].as_str() != Some("CLOSED") {
                return None;
            }
            let number = issue["number"].as_u64()?;
            let title = issue["title"].as_str()?.to_string();
            let updated_at = issue["updatedAt"].as_str().unwrap_or("").to_string();
            let body = issue["comments"]
                .as_array()
                .and_then(|comments| {
                    comments.iter().rev().find_map(|c| {
                        let text = c["body"].as_str()?;
                        text.contains("<!-- auto-worker-report -->")
                            .then_some(text.to_string())
                    })
                })
                .unwrap_or_else(|| "No worker report was posted for this issue.".to_string());
            Some(serde_json::json!({
                "issue_number": number,
                "title": title,
                "comment_body": body,
                "updated_at": updated_at,
            }))
        })
        .collect();
    Ok(Json(serde_json::to_value(reports).unwrap()))
}

// --- Storage/Git Operations ---

async fn get_session_commits(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"].as_str().unwrap_or_default();
    let session_id = args["sessionId"].as_str().unwrap_or_default();
    let project_uuid =
        uuid::Uuid::parse_str(project_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let session_uuid =
        uuid::Uuid::parse_str(session_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let (worktree_path, done_commits) = {
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
        match &session.worktree_path {
            Some(p) => (p.clone(), session.done_commits.clone()),
            None => {
                return Ok(Json(serde_json::to_value(&session.done_commits).unwrap()));
            }
        }
    };
    let wt = worktree_path.clone();
    let new_commits = tokio::task::spawn_blocking(move || {
        discover_branch_commits_server(&wt).unwrap_or_default()
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task failed: {}", e),
        )
    })?;
    let mut seen = HashSet::new();
    let mut all_commits = Vec::new();
    for c in new_commits.iter().chain(done_commits.iter()) {
        if seen.insert(c.hash.clone()) {
            all_commits.push(c.clone());
        }
    }
    if all_commits.len() > done_commits.len() {
        if let Ok(storage) = state.app.storage.lock() {
            if let Ok(mut project) = storage.load_project(project_uuid) {
                if let Some(s) = project.sessions.iter_mut().find(|s| s.id == session_uuid) {
                    s.done_commits = all_commits.clone();
                }
                let _ = storage.save_project(&project);
            }
        }
    }
    Ok(Json(serde_json::to_value(all_commits).unwrap()))
}

fn discover_branch_commits_server(worktree_path: &str) -> Result<Vec<models::CommitInfo>, String> {
    let repo = git2::Repository::discover(worktree_path)
        .map_err(|e| format!("Failed to open repo: {e}"))?;
    let head = repo.head().map_err(|e| format!("No HEAD: {e}"))?;
    let head_commit = head.peel_to_commit().map_err(|e| e.to_string())?;
    let main_oid = find_main_branch_oid_server(&repo);
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
        commits.push(models::CommitInfo { hash, message });
        if commits.len() >= 20 {
            break;
        }
    }
    Ok(commits)
}

fn find_main_branch_oid_server(repo: &git2::Repository) -> Option<git2::Oid> {
    for name in &["refs/heads/main", "refs/heads/master"] {
        if let Ok(reference) = repo.find_reference(name) {
            if let Ok(commit) = reference.peel_to_commit() {
                return Some(commit.id());
            }
        }
    }
    None
}

async fn save_session_prompt(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"].as_str().unwrap_or_default();
    let session_id = args["sessionId"].as_str().unwrap_or_default();
    let project_uuid =
        uuid::Uuid::parse_str(project_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let session_uuid =
        uuid::Uuid::parse_str(session_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let storage = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut project = storage
        .load_project(project_uuid)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let session = project
        .sessions
        .iter()
        .find(|s| s.id == session_uuid)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Session not found".to_string()))?;
    let prompt_text = session
        .initial_prompt
        .clone()
        .or_else(|| {
            session.github_issue.as_ref().map(|issue| {
                session_args::build_issue_prompt(issue.number, &issue.title, &issue.url, false)
            })
        })
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "Session has no prompt to save".to_string(),
            )
        })?;
    let name = {
        let truncated: String = prompt_text.chars().take(60).collect();
        if truncated.len() < prompt_text.len() {
            format!("{}...", truncated)
        } else {
            truncated
        }
    };
    let saved = models::SavedPrompt {
        id: uuid::Uuid::new_v4(),
        name,
        text: prompt_text,
        created_at: chrono::Utc::now().to_rfc3339(),
        source_session_label: session.label.clone(),
    };
    project.prompts.push(saved);
    storage
        .save_project(&project)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(Value::Null))
}

async fn list_project_prompts(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"].as_str().unwrap_or_default();
    let project_uuid =
        uuid::Uuid::parse_str(project_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let storage = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let project = storage
        .load_project(project_uuid)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::to_value(project.prompts).unwrap()))
}

async fn get_repo_head(Json(args): Json<Value>) -> Result<Json<Value>, (StatusCode, String)> {
    let repo_path = args["repoPath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing repoPath".to_string()))?
        .to_string();
    let result = tokio::task::spawn_blocking(move || {
        let repo = git2::Repository::open(&repo_path)
            .map_err(|e| format!("Failed to open repo: {}", e))?;
        let head = repo
            .head()
            .map_err(|e| format!("Failed to get HEAD: {}", e))?;
        let branch = head.shorthand().unwrap_or("HEAD").to_string();
        let commit = head
            .peel_to_commit()
            .map_err(|e| format!("Failed to peel to commit: {}", e))?;
        let short_hash = commit.id().to_string()[..7].to_string();
        Ok::<_, String>((branch, short_hash))
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task failed: {}", e),
        )
    })?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(result).unwrap()))
}

async fn get_session_token_usage(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"].as_str().unwrap_or_default();
    let session_id = args["sessionId"].as_str().unwrap_or_default();
    let project_uuid =
        uuid::Uuid::parse_str(project_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let session_uuid =
        uuid::Uuid::parse_str(session_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let (working_dir, kind) = {
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
        let dir = session
            .worktree_path
            .as_deref()
            .unwrap_or(&project.repo_path)
            .to_string();
        (dir, session.kind.clone())
    };
    let data =
        tokio::task::spawn_blocking(move || token_usage::get_token_usage(&working_dir, &kind))
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Task failed: {}", e),
                )
            })?
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(data).unwrap()))
}

// --- Directory Listing ---

async fn list_directories_at(Json(args): Json<Value>) -> Result<Json<Value>, (StatusCode, String)> {
    let path = args["path"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing path".to_string()))?
        .to_string();
    let p = Path::new(&path);
    if !p.is_dir() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Not a directory: {}", path),
        ));
    }
    // Restrict to directories under $HOME to prevent arbitrary filesystem enumeration
    let requested = std::fs::canonicalize(p).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("cannot resolve path: {}", e),
        )
    })?;
    let home = dirs::home_dir().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "cannot determine home directory".to_string(),
        )
    })?;
    if !requested.starts_with(&home) {
        return Err((
            StatusCode::FORBIDDEN,
            "path must be under the home directory".to_string(),
        ));
    }
    let entries = config::list_directories(p)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::to_value(entries).unwrap()))
}

async fn list_root_directories(
    AxumState(state): AxumState<Arc<ServerState>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let storage = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let base_dir = storage.base_dir();
    let cfg = config::load_config(&base_dir).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "No config found. Complete onboarding first.".to_string(),
        )
    })?;
    let entries = config::list_directories(Path::new(&cfg.projects_root))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::to_value(entries).unwrap()))
}

async fn generate_project_names(
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let description = args["description"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing description".to_string()))?
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

// --- Scaffold ---

async fn scaffold_project(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let name = args["name"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing name".to_string()))?
        .to_string();
    commands::validate_project_name(&name).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let repo_path = {
        let storage = state
            .app
            .storage
            .lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        if let Ok(inventory) = storage.list_projects() {
            if inventory.projects.iter().any(|p| p.name == name) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!("A project named '{}' already exists", name),
                ));
            }
        }
        let cfg = config::load_config(&storage.base_dir()).ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "No config found. Complete onboarding first.".to_string(),
            )
        })?;
        PathBuf::from(&cfg.projects_root).join(&name)
    };
    if repo_path.exists() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Directory already exists: {}", name),
        ));
    }
    let name_clone = name.clone();
    let project = tokio::task::spawn_blocking(move || {
        commands::scaffold_project_blocking(name_clone, repo_path)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task failed: {}", e),
        )
    })?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let storage = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if let Ok(inventory) = storage.list_projects() {
        if inventory.projects.iter().any(|p| p.name == project.name) {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("A project named '{}' already exists", project.name),
            ));
        }
    }
    storage
        .save_project(&project)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::to_value(project).unwrap()))
}

// --- Session Management ---

async fn stage_session() -> Result<Json<Value>, (StatusCode, String)> {
    Err((
        StatusCode::NOT_IMPLEMENTED,
        "stage_session requires AppHandle and is not available in server mode".to_string(),
    ))
}

async fn unstage_session(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"].as_str().unwrap_or_default();
    let project_uuid =
        uuid::Uuid::parse_str(project_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let storage = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut project = storage
        .load_project(project_uuid)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let staged = project.staged_session.take().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "No session is currently staged".to_string(),
        )
    })?;
    commands::kill_process_group(staged.pid);
    let _ = std::fs::remove_file(status_socket::staged_socket_path());
    storage
        .save_project(&project)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(Value::Null))
}

async fn submit_secure_env_value(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let request_id = args["requestId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing requestId".to_string()))?
        .to_string();
    let value = args["value"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing value".to_string()))?
        .to_string();
    let (pending, response_tx) = secure_env::take_secure_env_submission(&state.app, &request_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let req_id = request_id.clone();
    let result = tokio::task::spawn_blocking(move || {
        secure_env::update_env_file(&pending.env_path, &pending.key, &value)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task failed: {e}"),
        )
    })?;
    let result = secure_env::finish_secure_env_submission(&req_id, response_tx, result)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let status = if result.created { "created" } else { "updated" };
    Ok(Json(Value::String(status.to_string())))
}

async fn cancel_secure_env_request(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let request_id = args["requestId"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing requestId".to_string()))?
        .to_string();
    secure_env::cancel_secure_env_request(&state.app, &request_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

// --- Notes (additional) ---

async fn api_save_note_image(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let folder = args["folder"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing folder".to_string()))?
        .to_string();
    let extension = args["extension"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing extension".to_string()))?
        .to_string();
    let image_bytes: Vec<u8> = serde_json::from_value(args["imageBytes"].clone()).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("invalid imageBytes: {}", e),
        )
    })?;
    let base_dir = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    let filename = notes::save_note_image(&base_dir, &folder, &image_bytes, &extension)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(Value::String(filename)))
}

async fn api_resolve_note_asset_path(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let folder = args["folder"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing folder".to_string()))?
        .to_string();
    let relative_path = args["relativePath"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing relativePath".to_string()))?
        .to_string();
    let base_dir = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    let resolved = notes::resolve_note_asset_path(&base_dir, &folder, &relative_path)
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(Value::String(resolved)))
}

async fn api_duplicate_note(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let folder = args["folder"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing folder".to_string()))?
        .to_string();
    let filename = args["filename"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing filename".to_string()))?
        .to_string();
    let base_dir = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    let copy = notes::duplicate_note(&base_dir, &folder, &filename)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    server_try_commit(
        &state,
        &format!("duplicate {}/{} → {}", folder, filename, copy),
    );
    Ok(Json(serde_json::to_value(copy).unwrap()))
}

// --- Auth/Login ---

async fn start_claude_login(
    AxumState(state): AxumState<Arc<ServerState>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let session_id = uuid::Uuid::new_v4();
    let mut pty_manager = state
        .app
        .pty_manager
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    pty_manager
        .spawn_command(session_id, "claude", &["login"], state.app.emitter.clone())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::String(session_id.to_string())))
}

async fn stop_claude_login(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let session_id = args["sessionId"].as_str().unwrap_or_default();
    let id =
        uuid::Uuid::parse_str(session_id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let mut pty_manager = state
        .app
        .pty_manager
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    pty_manager
        .close_session(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(Value::Null))
}

// --- WebSocket ---

async fn ws_upgrade(
    ws: WebSocketUpgrade,
    AxumState(state): AxumState<Arc<ServerState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state.ws_tx.subscribe()))
}

async fn handle_ws(mut socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    loop {
        let msg = match rx.recv().await {
            Ok(msg) => msg,
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => break,
        };

        if socket.send(Message::Text(msg.into())).await.is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{auth_middleware, handle_ws, startup_messages, ServerState};
    use axum::{
        extract::{ws::WebSocketUpgrade, State as AxumState},
        middleware,
        response::IntoResponse,
        routing::{get, post},
        Router,
    };
    use futures_util::StreamExt;
    use once_cell::sync::Lazy;
    use reqwest::StatusCode;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use tokio::net::TcpListener;
    use tokio::sync::broadcast;
    use tokio::time::{timeout, Duration};
    use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

    use tempfile::TempDir;
    use the_controller_lib::{emitter::NoopEmitter, state::AppState, storage::Storage};

    static ENV_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    struct EnvGuard {
        key: &'static str,
        original: Option<String>,
    }

    impl EnvGuard {
        fn remove(key: &'static str) -> Self {
            let original = std::env::var(key).ok();
            std::env::remove_var(key);
            Self { key, original }
        }

        fn set(key: &'static str, value: &str) -> Self {
            let original = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, original }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.original {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }

    fn source_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
    }

    fn percent_encode_component(value: &str) -> String {
        let mut encoded = String::new();
        for byte in value.bytes() {
            if matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~') {
                encoded.push(byte as char);
            } else {
                encoded.push_str(&format!("%{byte:02X}"));
            }
        }
        encoded
    }

    async fn spawn_test_server(app: Router) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test listener should bind");
        let addr = listener
            .local_addr()
            .expect("test listener should expose local addr");
        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("test server should run");
        });
        (format!("http://{}", addr), handle)
    }

    fn extract_desktop_commands(source: &str) -> BTreeSet<String> {
        let invoke_list = source
            .split("tauri::generate_handler![")
            .nth(1)
            .and_then(|section| section.split("])").next())
            .expect("desktop invoke handler list should exist");

        invoke_list
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim().trim_end_matches(',');
                trimmed
                    .strip_prefix("commands::")
                    .or_else(|| trimmed.strip_prefix("deploy::commands::"))
                    .map(ToOwned::to_owned)
            })
            .collect()
    }

    fn extract_server_routes(source: &str) -> BTreeSet<String> {
        source
            .split("#[cfg(test)]")
            .next()
            .expect("server source should have a non-test section")
            .split(".route(")
            .skip(1)
            .filter_map(|segment| {
                let start = segment.find('"')?;
                let after_quote = &segment[start + 1..];
                let end = after_quote.find('"')?;
                let path = &after_quote[..end];
                path.strip_prefix("/api/").map(ToOwned::to_owned)
            })
            .collect()
    }

    #[test]
    fn server_routes_cover_desktop_command_surface() {
        let root = source_root();
        let lib_rs = fs::read_to_string(root.join("lib.rs")).expect("should read lib.rs");
        let server_rs =
            fs::read_to_string(root.join("server/main.rs")).expect("should read server.rs");

        let desktop_commands = extract_desktop_commands(&lib_rs);
        let server_routes = extract_server_routes(&server_rs);
        let allowed_server_only = BTreeSet::from([String::from("list_archived_projects")]);

        let missing: Vec<_> = desktop_commands
            .difference(&server_routes)
            .cloned()
            .collect();
        let unexpected_server_only: Vec<_> = server_routes
            .difference(&desktop_commands)
            .filter(|name| !allowed_server_only.contains(*name))
            .cloned()
            .collect();

        assert!(
            missing.is_empty(),
            "server router is missing desktop commands: {:?}",
            missing
        );
        assert!(
            unexpected_server_only.is_empty(),
            "server router has unexpected command routes: {:?}",
            unexpected_server_only
        );
    }

    #[test]
    fn authenticated_startup_message_does_not_print_raw_token() {
        let addr = "127.0.0.1:3001";
        let token = "super-secret-token";

        let (stdout_message, log_message) = startup_messages(addr, Some(token));

        let stdout_message = stdout_message.expect("authenticated startup should print guidance");
        assert!(stdout_message.contains(addr));
        assert!(
            !stdout_message.contains(token),
            "stdout should not contain raw auth token"
        );
        assert!(
            stdout_message.contains("server.env"),
            "stdout should direct operators to the config file"
        );

        assert!(log_message.contains(addr));
        assert!(!log_message.contains(token));
    }

    #[tokio::test]
    async fn unauthenticated_cross_origin_api_requests_are_forbidden() {
        let _env_lock = ENV_LOCK.lock().expect("env lock should not be poisoned");
        let _auth_guard = EnvGuard::remove("CONTROLLER_AUTH_TOKEN");
        let app = Router::new()
            .route("/api/ping", post(|| async { StatusCode::OK }))
            .layer(middleware::from_fn(auth_middleware));
        let (base_url, server_handle) = spawn_test_server(app).await;

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{base_url}/api/ping"))
            .header("Origin", "https://evil.example")
            .send()
            .await
            .expect("cross-origin request should complete");

        assert_eq!(response.status(), reqwest::StatusCode::FORBIDDEN);
        server_handle.abort();
    }

    #[tokio::test]
    async fn unauthenticated_same_origin_api_requests_are_allowed() {
        let _env_lock = ENV_LOCK.lock().expect("env lock should not be poisoned");
        let _auth_guard = EnvGuard::remove("CONTROLLER_AUTH_TOKEN");
        let app = Router::new()
            .route("/api/ping", post(|| async { StatusCode::OK }))
            .layer(middleware::from_fn(auth_middleware));
        let (base_url, server_handle) = spawn_test_server(app).await;

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{base_url}/api/ping"))
            .header("Origin", &base_url)
            .send()
            .await
            .expect("same-origin request should complete");

        assert_eq!(response.status(), reqwest::StatusCode::OK);
        server_handle.abort();
    }

    #[tokio::test]
    async fn websocket_auth_accepts_percent_encoded_token_query() {
        let _env_lock = ENV_LOCK.lock().expect("env lock should not be poisoned");

        async fn test_ws_upgrade(
            ws: WebSocketUpgrade,
            AxumState(state): AxumState<Arc<ServerState>>,
        ) -> impl IntoResponse {
            ws.write_buffer_size(0)
                .on_upgrade(move |socket| handle_ws(socket, state.ws_tx.subscribe()))
        }

        let token = "abc/+%=&token";
        let _auth_guard = EnvGuard::set("CONTROLLER_AUTH_TOKEN", token);
        let temp_dir = TempDir::new().expect("temp dir should be created");
        let app_state = Arc::new(
            AppState::from_storage(
                Storage::new(temp_dir.path().to_path_buf()),
                NoopEmitter::new(),
            )
            .expect("test app state should initialize"),
        );
        let (ws_tx, _) = broadcast::channel(8);
        let state = Arc::new(ServerState {
            app: app_state,
            ws_tx: ws_tx.clone(),
        });
        let app = Router::new()
            .route("/api/ping", post(|| async { StatusCode::OK }))
            .route("/ws", get(test_ws_upgrade))
            .layer(middleware::from_fn(auth_middleware))
            .with_state(state);
        let (base_url, server_handle) = spawn_test_server(app).await;

        let client = reqwest::Client::new();
        let api_response = client
            .post(format!("{base_url}/api/ping"))
            .bearer_auth(token)
            .send()
            .await
            .expect("authenticated API request should complete");
        assert_eq!(api_response.status(), reqwest::StatusCode::OK);

        let ws_url = format!(
            "{}/ws?token={}",
            base_url.replacen("http", "ws", 1),
            percent_encode_component(token)
        );
        let (mut socket, _) = connect_async(&ws_url)
            .await
            .expect("websocket client should connect with percent-encoded token");

        let expected = "authenticated".to_string();
        ws_tx
            .send(expected.clone())
            .expect("event should be broadcast");
        let received = timeout(Duration::from_secs(2), socket.next())
            .await
            .expect("authenticated websocket should receive an event before timeout")
            .expect("socket should remain open after auth")
            .expect("frame should decode");
        assert_eq!(
            received.into_text().expect("frame should be text"),
            expected
        );

        server_handle.abort();
    }

    #[tokio::test]
    async fn websocket_client_recovers_after_broadcast_lag() {
        async fn test_ws_upgrade(
            ws: WebSocketUpgrade,
            AxumState(state): AxumState<Arc<ServerState>>,
        ) -> impl IntoResponse {
            ws.write_buffer_size(0)
                .on_upgrade(move |socket| handle_ws(socket, state.ws_tx.subscribe()))
        }

        let temp_dir = TempDir::new().expect("temp dir should be created");
        let app_state = Arc::new(
            AppState::from_storage(
                Storage::new(temp_dir.path().to_path_buf()),
                NoopEmitter::new(),
            )
            .expect("test app state should initialize"),
        );
        let (ws_tx, _) = broadcast::channel(2);
        let state = Arc::new(ServerState {
            app: app_state,
            ws_tx: ws_tx.clone(),
        });
        let app = Router::new()
            .route("/ws", get(test_ws_upgrade))
            .with_state(state);
        let (base_url, server_handle) = spawn_test_server(app).await;

        let ws_url = format!("{}/ws", base_url.replacen("http", "ws", 1));
        let (mut socket, _) = connect_async(&ws_url)
            .await
            .expect("websocket client should connect");

        ws_tx
            .send("ready".to_string())
            .expect("ready event should be broadcast");
        let ready = timeout(Duration::from_secs(2), socket.next())
            .await
            .expect("ready event should arrive before timeout")
            .expect("socket should remain open before lag")
            .expect("ready frame should decode");
        assert_eq!(
            ready.into_text().expect("ready frame should be text"),
            "ready"
        );

        let flood_payload = "x".repeat(256 * 1024);
        for index in 0..64 {
            ws_tx
                .send(format!("flood-{index}:{flood_payload}"))
                .expect("flood event should be broadcast");
        }
        let recovery = "recovery".to_string();
        ws_tx
            .send(recovery.clone())
            .expect("recovery event should be broadcast");

        let recovered = timeout(Duration::from_secs(5), async {
            loop {
                match socket.next().await {
                    Some(Ok(WsMessage::Text(text))) if text == recovery => return true,
                    Some(Ok(_)) => continue,
                    Some(Err(error)) => {
                        panic!("websocket should stay connected after lag: {error}")
                    }
                    None => return false,
                }
            }
        })
        .await
        .expect("recovery event should arrive before timeout");

        assert!(
            recovered,
            "socket closed after lag instead of receiving later events"
        );
        server_handle.abort();
    }
}
