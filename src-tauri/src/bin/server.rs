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
use the_controller_lib::{
    controller_chat::{self, ControllerFocusUpdate},
    config,
    emitter::WsBroadcastEmitter,
    notes,
    state::AppState,
};
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
        .route("/api/write_to_pty", post(write_to_pty))
        .route("/api/send_raw_to_pty", post(send_raw_to_pty))
        .route("/api/resize_pty", post(resize_pty))
        .route("/api/close_session", post(close_session))
        .route("/api/create_session", post(create_session))
        .route("/api/merge_session_branch", post(merge_session_branch))
        .route("/api/get_controller_chat_session", post(get_controller_chat_session))
        .route("/api/update_controller_chat_focus", post(update_controller_chat_focus))
        .route("/api/send_controller_chat_message", post(send_controller_chat_message))
        .route("/api/list_notes", post(api_list_notes))
        .route("/api/read_note", post(api_read_note))
        .route("/api/write_note", post(api_write_note))
        .route("/api/create_note", post(api_create_note))
        .route("/api/delete_note", post(api_delete_note))
        .route("/api/rename_note", post(api_rename_note))
        .route("/ws", get(ws_upgrade))
        .fallback(post(fallback_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    println!("Server listening on http://localhost:3001");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn fallback_handler() -> Json<Value> {
    Json(Value::Null)
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
    let session_id = args["sessionId"]
        .as_str()
        .unwrap_or_default();
    let rows = args["rows"].as_u64().unwrap_or(24) as u16;
    let cols = args["cols"].as_u64().unwrap_or(80) as u16;
    let id = uuid::Uuid::parse_str(session_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

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
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Task failed: {}", e)))??;

    Ok(Json(Value::Null))
}

async fn load_project(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .unwrap_or_default();
    let id = uuid::Uuid::parse_str(project_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let storage = state
        .app
        .storage
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let project = storage
        .load_project(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::to_value(project).unwrap()))
}

async fn write_to_pty(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let session_id = args["sessionId"]
        .as_str()
        .unwrap_or_default();
    let data = args["data"].as_str().unwrap_or_default();
    let id = uuid::Uuid::parse_str(session_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
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
    let session_id = args["sessionId"]
        .as_str()
        .unwrap_or_default();
    let data = args["data"].as_str().unwrap_or_default();
    let id = uuid::Uuid::parse_str(session_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
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
    let session_id = args["sessionId"]
        .as_str()
        .unwrap_or_default();
    let rows = args["rows"].as_u64().unwrap_or(24) as u16;
    let cols = args["cols"].as_u64().unwrap_or(80) as u16;
    let id = uuid::Uuid::parse_str(session_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
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
    let session_id = args["sessionId"]
        .as_str()
        .unwrap_or_default();
    let id = uuid::Uuid::parse_str(session_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let mut pty = state
        .app
        .pty_manager
        .lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let _ = pty.close_session(id);
    Ok(Json(Value::Null))
}

async fn create_session(
    AxumState(_state): AxumState<Arc<ServerState>>,
    Json(_args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    Err((
        StatusCode::NOT_IMPLEMENTED,
        "create_session not yet wired".to_string(),
    ))
}

async fn merge_session_branch(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    use the_controller_lib::worktree::{MergeResult, WorktreeManager};
    use the_controller_lib::models::MergeResponse;

    let project_id = args["projectId"].as_str().unwrap_or_default();
    let session_id = args["sessionId"].as_str().unwrap_or_default();
    let project_uuid = uuid::Uuid::parse_str(project_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let session_uuid = uuid::Uuid::parse_str(session_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let (repo_path, worktree_path, branch_name) = {
        let storage = state.app.storage.lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let project = storage.load_project(project_uuid)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let session = project.sessions.iter().find(|s| s.id == session_uuid)
            .ok_or_else(|| (StatusCode::NOT_FOUND, "Session not found".to_string()))?;
        let wt_path = session.worktree_path.clone()
            .ok_or_else(|| (StatusCode::BAD_REQUEST, "Session has no worktree".to_string()))?;
        let branch = session.worktree_branch.clone()
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
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Task failed: {}", e)))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

        match result {
            MergeResult::PrCreated(url) => {
                let resp = MergeResponse::PrCreated { url };
                return Ok(Json(serde_json::to_value(resp).unwrap()));
            }
            MergeResult::RebaseConflicts => {
                let prompt = "merge\r";
                {
                    let mut pty_manager = state.app.pty_manager.lock()
                        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                    let _ = pty_manager.write_to_session(session_uuid, prompt.as_bytes());
                }

                let _ = state.app.emitter.emit(
                    "merge-status",
                    &format!("Rebase conflicts (attempt {}/{}). Claude is resolving...", attempt + 1, MAX_RETRIES),
                );

                let wt_poll = worktree_path.clone();
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
                    let wt_check = wt_poll.clone();
                    let still_rebasing = tokio::task::spawn_blocking(move || {
                        WorktreeManager::is_rebase_in_progress(&wt_check)
                    })
                    .await
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Task failed: {}", e)))?;
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
        format!("Merge failed after {} attempts due to recurring conflicts", MAX_RETRIES),
    ))
}

async fn get_controller_chat_session(
    AxumState(state): AxumState<Arc<ServerState>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let session = controller_chat::get_controller_chat_session(&state.app)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(session).unwrap()))
}

async fn update_controller_chat_focus(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"]
        .as_str()
        .map(|id| uuid::Uuid::parse_str(id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string())))
        .transpose()?;
    let session_id = args["sessionId"]
        .as_str()
        .map(|id| uuid::Uuid::parse_str(id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string())))
        .transpose()?;

    let session = controller_chat::update_focus_snapshot(
        &state.app,
        ControllerFocusUpdate {
            project_id,
            project_name: args["projectName"].as_str().map(str::to_string),
            session_id,
            note_filename: args["noteFilename"].as_str().map(str::to_string),
            workspace_mode: args["workspaceMode"].as_str().map(str::to_string),
        },
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::to_value(session).unwrap()))
}

async fn send_controller_chat_message(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let message = args["message"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing message".to_string()))?
        .to_string();

    let session = controller_chat::send_message(&state.app, message)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(session).unwrap()))
}

// --- Notes ---

async fn api_list_notes(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_name = args["projectName"]
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing projectName".to_string()))?
        .to_string();
    let base_dir = state.app.storage.lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    let entries = notes::list_notes(&base_dir, &project_name)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::to_value(entries).unwrap()))
}

async fn api_read_note(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_name = args["projectName"].as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing projectName".to_string()))?.to_string();
    let filename = args["filename"].as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing filename".to_string()))?.to_string();
    let base_dir = state.app.storage.lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    let content = notes::read_note(&base_dir, &project_name, &filename)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::to_value(content).unwrap()))
}

async fn api_write_note(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_name = args["projectName"].as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing projectName".to_string()))?.to_string();
    let filename = args["filename"].as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing filename".to_string()))?.to_string();
    let content = args["content"].as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing content".to_string()))?.to_string();
    let base_dir = state.app.storage.lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    notes::write_note(&base_dir, &project_name, &filename, &content)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(Value::Null))
}

async fn api_create_note(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_name = args["projectName"].as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing projectName".to_string()))?.to_string();
    let title = args["title"].as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing title".to_string()))?.to_string();
    let base_dir = state.app.storage.lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    let filename = notes::create_note(&base_dir, &project_name, &title)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::to_value(filename).unwrap()))
}

async fn api_delete_note(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_name = args["projectName"].as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing projectName".to_string()))?.to_string();
    let filename = args["filename"].as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing filename".to_string()))?.to_string();
    let base_dir = state.app.storage.lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    notes::delete_note(&base_dir, &project_name, &filename)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(Value::Null))
}

async fn api_rename_note(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_name = args["projectName"].as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing projectName".to_string()))?.to_string();
    let old_name = args["oldName"].as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing oldName".to_string()))?.to_string();
    let new_name = args["newName"].as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing newName".to_string()))?.to_string();
    let base_dir = state.app.storage.lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    let filename = notes::rename_note(&base_dir, &project_name, &old_name, &new_name)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::to_value(filename).unwrap()))
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
