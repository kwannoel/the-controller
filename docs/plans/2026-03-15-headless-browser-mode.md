# Headless Browser Mode Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Run The Controller on a headless Linux server, accessible via web browser, while keeping the Tauri desktop app fully functional.

**Architecture:** The existing `server.rs` (Axum) + `backend.ts` (Tauri/HTTP abstraction) already provide ~60% of the plumbing. The work is: (1) wire all missing API routes in the server, (2) serve the built frontend as static files, (3) decouple `status_socket` and schedulers from `AppHandle`, (4) guard Tauri-only frontend imports behind `isTauri`, (5) add token-based auth for network safety, (6) add graceful shutdown with tmux cleanup.

**Tech Stack:** Rust (Axum, tokio, tower-http), Svelte 5, Vite, xterm.js

---

## Current State

**Already working:**
- `src-tauri/src/bin/server.rs` — Axum HTTP/WS server on port 3001 with ~20 routes
- `src/lib/backend.ts` — `command()` routes to Tauri IPC or `fetch(/api/...)`, `listen()` routes to Tauri events or WebSocket
- `src-tauri/src/emitter.rs` — `WsBroadcastEmitter` sends events over broadcast channel → WebSocket
- `vite.config.ts` — dev proxy for `/api` and `/ws` to `localhost:3001`

**Gaps identified:**

| Gap | Severity | Description |
|-----|----------|-------------|
| Missing server routes | Critical | `create_session`, `create_project`, `delete_project`, `scaffold_project`, `get_agents_md`, `update_agents_md`, `set_initial_prompt`, `close_session` (PTY kill), GitHub issues, maintainer, auto-worker, token usage, deploy, etc. |
| Static file serving | Critical | Server doesn't serve the Vite-built `dist/` — browser gets nothing |
| `status_socket` coupled to `AppHandle` | Critical | `start_listener` takes `AppHandle`; server can't use it |
| Frontend Tauri imports | High | `main.ts`, `App.svelte`, `Terminal.svelte`, `clipboard.ts` import `@tauri-apps/*` directly — crashes in browser |
| No authentication | High | HTTP server is wide open — dangerous on a network |
| No graceful shutdown | Medium | Server doesn't clean up tmux sessions on SIGTERM |
| Schedulers coupled to `AppHandle` | Medium | `MaintainerScheduler` and `AutoWorkerScheduler` use `AppHandle` |
| No CLI for server | Low | No way to configure port, bind address, auth token |

---

## Task 1: Decouple `status_socket::start_listener` from `AppHandle`

The status socket listener currently takes `AppHandle` to get `AppState`. The server binary has `AppState` directly but no `AppHandle`. We need a version that accepts `AppState` + emitter directly.

**Files:**
- Modify: `src-tauri/src/status_socket.rs`
- Modify: `src-tauri/src/lib.rs` (call site)
- Test: `cd src-tauri && cargo test status_socket`

**Step 1: Add `start_listener_with_state` function**

In `status_socket.rs`, add a new public function that takes `Arc<AppState>` instead of `AppHandle`:

```rust
/// Start the Unix domain socket listener using an AppState directly.
/// Used by the standalone server binary (no Tauri AppHandle available).
pub fn start_listener_with_state(state: Arc<AppState>) {
    let path = socket_path();

    if std::path::Path::new(&path).exists() {
        match UnixStream::connect(&path) {
            Ok(_) => {
                eprintln!(
                    "Warning: another instance appears to be running (socket {} is active)",
                    path
                );
                return;
            }
            Err(_) => {
                let _ = std::fs::remove_file(&path);
            }
        }
    }

    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind Unix socket at {}: {}", path, e);
            return;
        }
    };

    let emitter = state.emitter.clone();

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let state = state.clone();
                    let emitter = emitter.clone();
                    std::thread::spawn(move || {
                        handle_connection_with_state(stream, &state, &emitter);
                    });
                }
                Err(e) => {
                    eprintln!("Error accepting connection on status socket: {}", e);
                }
            }
        }
    });
}
```

**Step 2: Extract `handle_connection_with_state`**

Refactor `handle_connection` to have a version that takes `&AppState` instead of `&AppHandle`. The existing `handle_connection` can delegate to it:

```rust
fn handle_connection_with_state(
    stream: UnixStream,
    state: &AppState,
    emitter: &Arc<dyn EventEmitter>,
) {
    let mut writer = match stream.try_clone() {
        Ok(stream) => stream,
        Err(err) => {
            eprintln!("Failed to clone status socket stream: {}", err);
            return;
        }
    };
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        match line {
            Ok(msg) => {
                let msg = msg.trim();
                match parse_socket_message(msg) {
                    Ok(SocketMessage::Status { status, session_id }) => {
                        if status == "cleanup" {
                            handle_cleanup_with_state(state, session_id);
                            return;
                        }
                        let event_name = format!("session-status-hook:{}", session_id);
                        if let Err(e) = emitter.emit(&event_name, &status) {
                            eprintln!("Failed to emit {}: {}", event_name, e);
                        }
                        if status == "idle" {
                            crate::auto_worker::notify_session_idle(session_id);
                        }
                    }
                    Ok(SocketMessage::SecureEnv(request)) => {
                        let (response_tx, response_rx) = std::sync::mpsc::sync_channel(1);
                        match dispatch_secure_env_request(state, emitter, request, response_tx) {
                            Ok(()) => match response_rx.recv() {
                                Ok(response) => write_socket_response(&mut writer, &response),
                                Err(err) => {
                                    eprintln!("Failed to receive secure env response: {}", err);
                                    write_socket_response(
                                        &mut writer,
                                        &crate::secure_env::SecureEnvResponse {
                                            kind: crate::secure_env::SecureEnvResponseKind::Error,
                                            status: "response-channel-closed".to_string(),
                                            request_id: "unknown".to_string(),
                                        },
                                    );
                                }
                            },
                            Err(response) => write_socket_response(&mut writer, &response),
                        }
                        return;
                    }
                    Err(err) if msg.starts_with("secure-env:") => {
                        eprintln!("Invalid secure env socket message: {}", err);
                        write_socket_response(
                            &mut writer,
                            &crate::secure_env::SecureEnvResponse {
                                kind: crate::secure_env::SecureEnvResponseKind::Error,
                                status: "invalid-request".to_string(),
                                request_id: "unknown".to_string(),
                            },
                        );
                        return;
                    }
                    Err(_) => {}
                }
            }
            Err(e) => {
                eprintln!("Error reading from status socket connection: {}", e);
                break;
            }
        }
    }
}
```

Also add `handle_cleanup_with_state` that takes `&AppState` directly (extract from `handle_cleanup`).

**Step 3: Refactor existing `start_listener` to delegate**

Make the existing `start_listener(AppHandle)` extract `AppState` and call `start_listener_with_state`:

```rust
pub fn start_listener(app_handle: AppHandle) {
    let state = match app_handle.try_state::<AppState>() {
        Some(s) => {
            // Clone into Arc for the new function — AppState is already managed by Tauri
            // We need to wrap it. Actually, Tauri's State is already Arc-like.
            // For now, keep the AppHandle version working as-is but share the handler logic.
        }
        None => {
            eprintln!("status_socket: AppState not available");
            return;
        }
    };
    // Keep existing implementation but have handle_connection delegate to handle_connection_with_state
}
```

The key insight: `handle_connection` currently takes `&AppHandle` only to call `app_handle.try_state::<AppState>()`. Refactor it to extract `AppState` once at the top of `start_listener` and pass it down.

**Step 4: Run tests**

```bash
cd src-tauri && cargo test status_socket
cd src-tauri && cargo clippy -- -D warnings
```

**Step 5: Commit**

```bash
git add src-tauri/src/status_socket.rs src-tauri/src/lib.rs
git commit -m "refactor: decouple status_socket from AppHandle"
```

---

## Task 2: Serve static frontend files from the server

The server needs to serve the Vite-built `dist/` directory so browsers can load the app.

**Files:**
- Modify: `src-tauri/src/bin/server.rs`
- Modify: `src-tauri/Cargo.toml` (add `tower-http` serve-dir feature)

**Step 1: Add `tower-http` ServeDir feature**

In `Cargo.toml`, update the `tower-http` dependency:

```toml
tower-http = { version = "0.6", features = ["cors", "fs"], optional = true }
```

**Step 2: Add static file serving with SPA fallback**

In `server.rs`, after all API routes, add a fallback that serves from `dist/`:

```rust
use tower_http::services::{ServeDir, ServeFile};

// In main(), after building the router:
let dist_dir = std::env::var("CONTROLLER_DIST_DIR")
    .unwrap_or_else(|_| {
        // Default: look for dist/ relative to the binary
        let exe = std::env::current_exe().unwrap_or_default();
        exe.parent()
            .unwrap_or(Path::new("."))
            .join("dist")
            .to_string_lossy()
            .to_string()
    });

let serve_dir = ServeDir::new(&dist_dir)
    .not_found_service(ServeFile::new(format!("{}/index.html", dist_dir)));

let app = Router::new()
    // ... all API routes ...
    .fallback_service(serve_dir)  // replaces the old fallback(post(fallback_handler))
    .layer(CorsLayer::permissive())
    .with_state(state);
```

This serves static files and falls back to `index.html` for SPA routing.

**Step 3: Build and test manually**

```bash
pnpm build                                    # builds dist/
cd src-tauri && cargo build --features server  # builds server binary
CONTROLLER_DIST_DIR=../../dist ./target/debug/server
# Open http://localhost:3001 in browser — should see the app
```

**Step 4: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/bin/server.rs
git commit -m "feat(server): serve static frontend files with SPA fallback"
```

---

## Task 3: Guard frontend Tauri-only imports

Several frontend files import `@tauri-apps/*` at the top level, which crashes in a pure browser environment (no `__TAURI_INTERNALS__`). These need to be lazy-imported or guarded.

**Files:**
- Modify: `src/main.ts`
- Modify: `src/lib/Terminal.svelte`
- Modify: `src/lib/AgentDashboard.svelte`
- Modify: `src/lib/IssuesModal.svelte`
- Modify: `src/lib/clipboard.ts`
- Test: `pnpm test`

**Step 1: Fix `main.ts`**

Replace the top-level `import { invoke }` with a conditional:

```typescript
import App from "./App.svelte";
import "./app.css";
import { mount } from "svelte";

const isTauri = !!(window as any).__TAURI_INTERNALS__;

function logToBackend(message: string) {
  if (isTauri) {
    import("@tauri-apps/api/core").then(({ invoke }) => {
      invoke("log_frontend_error", { message }).catch(() => {});
    });
  } else {
    // In browser mode, log to console (server can add a /api/log_frontend_error later)
    console.error("[frontend]", message);
  }
}

window.addEventListener("error", (e) => {
  const loc = e.filename ? ` at ${e.filename}:${e.lineno}:${e.colno}` : "";
  logToBackend(`${e.message}${loc}\n${e.error?.stack || ""}`);
});

window.addEventListener("unhandledrejection", (e) => {
  const reason = e.reason instanceof Error
    ? `${e.reason.message}\n${e.reason.stack}`
    : String(e.reason);
  logToBackend(`Unhandled rejection: ${reason}`);
});

const app = mount(App, {
  target: document.getElementById("app")!,
});

export default app;
```

**Step 2: Fix `openUrl` usages in Terminal.svelte, AgentDashboard.svelte, IssuesModal.svelte**

Replace static `import { openUrl } from "@tauri-apps/plugin-opener"` with a helper:

Create `src/lib/platform.ts`:

```typescript
const isTauri = typeof window !== "undefined" && !!(window as any).__TAURI_INTERNALS__;

export async function openUrl(url: string): Promise<void> {
  if (isTauri) {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    return openUrl(url);
  }
  window.open(url, "_blank", "noopener");
}

export { isTauri };
```

Then update the three components to import from `$lib/platform` instead of `@tauri-apps/plugin-opener`.

**Step 3: Fix `clipboard.ts`**

Guard the `readImage` import:

```typescript
const isTauri = typeof window !== "undefined" && !!(window as any).__TAURI_INTERNALS__;

export async function readClipboardImage(): Promise<Uint8Array | null> {
  if (!isTauri) return null;
  const { readImage } = await import("@tauri-apps/plugin-clipboard-manager");
  // ... rest of existing logic
}
```

**Step 4: Run tests**

```bash
pnpm test
pnpm check
```

**Step 5: Commit**

```bash
git add src/main.ts src/lib/platform.ts src/lib/Terminal.svelte \
  src/lib/AgentDashboard.svelte src/lib/IssuesModal.svelte src/lib/clipboard.ts
git commit -m "fix: guard Tauri-only imports for browser compatibility"
```

---

## Task 4: Wire critical missing server routes

The server is missing several routes that are essential for basic operation. Wire them in priority order.

**Files:**
- Modify: `src-tauri/src/bin/server.rs`
- Test: manual browser testing + `cargo build --features server`

**Step 1: Wire `create_session`**

This is the most critical missing route. Port the logic from `commands.rs:593-707`:

```rust
async fn create_session(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"].as_str().unwrap_or_default();
    let kind = args["kind"].as_str().unwrap_or("claude").to_string();
    let background = args["background"].as_bool().unwrap_or(false);
    let initial_prompt = args["initialPrompt"].as_str().map(|s| s.to_string());
    let github_issue: Option<the_controller_lib::models::GithubIssue> =
        serde_json::from_value(args["githubIssue"].clone()).ok();

    let project_uuid = uuid::Uuid::parse_str(project_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let session_id = uuid::Uuid::new_v4();

    // Load project and generate label
    let (repo_path, label, base_dir, project_name) = {
        let storage = state.app.storage.lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let project = storage.load_project(project_uuid)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let label = the_controller_lib::commands::next_session_label(&project.sessions);
        (project.repo_path.clone(), label, storage.base_dir(), project.name.clone())
    };

    // Create worktree
    use the_controller_lib::worktree::WorktreeManager;
    let worktree_dir = base_dir.join("worktrees").join(&project_name).join(&label);

    let (session_dir, wt_path, wt_branch) = {
        let rp = repo_path.clone();
        let lb = label.clone();
        let wd = worktree_dir.clone();
        tokio::task::spawn_blocking(move || {
            match WorktreeManager::create_worktree(&rp, &lb, &wd) {
                Ok(worktree_path) => {
                    let wt_str = worktree_path.to_string_lossy().to_string();
                    Ok((wt_str.clone(), Some(wt_str), Some(lb)))
                }
                Err(e) if e == "unborn_branch" => Ok((rp, None, None)),
                Err(e) => Err(e),
            }
        })
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Task failed: {}", e)))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
    };

    // Build initial prompt
    let initial_prompt = initial_prompt.or_else(|| {
        github_issue.as_ref().map(|issue| {
            the_controller_lib::session_args::build_issue_prompt(
                issue.number, &issue.title, &issue.url, background,
            )
        })
    });

    let session_config = the_controller_lib::models::SessionConfig {
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

    // Save session to project
    {
        let storage = state.app.storage.lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let mut project = storage.load_project(project_uuid)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        project.sessions.push(session_config);
        storage.save_project(&project)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    // Spawn PTY
    let pty_manager = state.app.pty_manager.clone();
    let emitter = state.app.emitter.clone();
    let sd = session_dir.clone();
    let k = kind.clone();
    let ip = initial_prompt.clone();
    tokio::task::spawn_blocking(move || {
        let mut mgr = pty_manager.lock().map_err(|e| e.to_string())?;
        mgr.spawn_session(session_id, &sd, &k, emitter, false, ip.as_deref(), 24, 80)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Task failed: {}", e)))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::to_value(session_id.to_string()).unwrap()))
}
```

**Step 2: Wire `create_project`**

```rust
async fn create_project(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let name = args["name"].as_str().unwrap_or_default().to_string();
    let repo_path = args["repoPath"].as_str().unwrap_or_default().to_string();

    the_controller_lib::commands::validate_project_name(&name)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    if !std::path::Path::new(&repo_path).is_dir() {
        return Err((StatusCode::BAD_REQUEST, format!("not a directory: {}", repo_path)));
    }

    let storage = state.app.storage.lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if let Ok(inventory) = storage.list_projects() {
        if inventory.projects.iter().any(|p| p.name == name) {
            return Err((StatusCode::CONFLICT, format!("Project '{}' already exists", name)));
        }
    }

    let project = the_controller_lib::models::Project {
        id: uuid::Uuid::new_v4(),
        name: name.clone(),
        repo_path: repo_path.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        archived: false,
        maintainer: the_controller_lib::models::MaintainerConfig::default(),
        auto_worker: the_controller_lib::models::AutoWorkerConfig::default(),
        prompts: vec![],
        sessions: vec![],
        staged_session: None,
    };

    storage.save_project(&project).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let repo_agents = std::path::Path::new(&repo_path).join("agents.md");
    if !repo_agents.exists() {
        storage.save_agents_md(project.id, &the_controller_lib::commands::render_agents_md(&name))
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    the_controller_lib::commands::ensure_claude_md_symlink(std::path::Path::new(&repo_path))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::to_value(project).unwrap()))
}
```

**Step 3: Wire remaining essential routes**

Add these simpler routes (they mostly just proxy to storage):

```rust
// delete_project
async fn delete_project(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"].as_str().unwrap_or_default();
    let delete_repo = args["deleteRepo"].as_bool().unwrap_or(false);
    let id = uuid::Uuid::parse_str(project_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let storage = state.app.storage.lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let project = storage.load_project(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    {
        let mut pty_manager = state.app.pty_manager.lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        for session in &project.sessions {
            let _ = pty_manager.close_session(session.id);
        }
    }

    storage.delete_project_dir(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if delete_repo && std::path::Path::new(&project.repo_path).exists() {
        std::fs::remove_dir_all(&project.repo_path)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to delete repo: {}", e)))?;
    }

    Ok(Json(Value::Null))
}

// get_agents_md
async fn get_agents_md(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"].as_str().unwrap_or_default();
    let id = uuid::Uuid::parse_str(project_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let storage = state.app.storage.lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let project = storage.load_project(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let content = storage.get_agents_md(&project)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::to_value(content).unwrap()))
}

// update_agents_md
async fn update_agents_md(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"].as_str().unwrap_or_default();
    let content = args["content"].as_str().unwrap_or_default().to_string();
    let id = uuid::Uuid::parse_str(project_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let storage = state.app.storage.lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    storage.save_agents_md(id, &content)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(Value::Null))
}

// set_initial_prompt
async fn set_initial_prompt(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let project_id = args["projectId"].as_str().unwrap_or_default();
    let session_id = args["sessionId"].as_str().unwrap_or_default();
    let prompt = args["prompt"].as_str().unwrap_or_default().to_string();
    let project_uuid = uuid::Uuid::parse_str(project_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let session_uuid = uuid::Uuid::parse_str(session_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let storage = state.app.storage.lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut project = storage.load_project(project_uuid)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if let Some(session) = project.sessions.iter_mut().find(|s| s.id == session_uuid) {
        if session.initial_prompt.is_none() {
            session.initial_prompt = Some(prompt);
            storage.save_project(&project)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
    }
    Ok(Json(Value::Null))
}

// check_claude_cli
async fn check_claude_cli() -> Result<Json<Value>, (StatusCode, String)> {
    let result = tokio::task::spawn_blocking(|| {
        the_controller_lib::config::check_claude_cli_status()
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Task failed: {}", e)))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(result).unwrap()))
}

// home_dir
async fn home_dir() -> Result<Json<Value>, (StatusCode, String)> {
    let dir = dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    Ok(Json(serde_json::to_value(dir).unwrap()))
}

// save_onboarding_config
async fn save_onboarding_config(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let projects_root = args["projectsRoot"].as_str().unwrap_or_default().to_string();
    let default_provider = args["defaultProvider"].as_str().unwrap_or("claude").to_string();
    let base_dir = state.app.storage.lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    the_controller_lib::config::save_config(
        &base_dir,
        &the_controller_lib::config::AppConfig { projects_root, default_provider },
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(Value::Null))
}

// log_frontend_error (just log to stderr)
async fn log_frontend_error(Json(args): Json<Value>) -> Json<Value> {
    if let Some(message) = args["message"].as_str() {
        eprintln!("[frontend] {}", message);
    }
    Json(Value::Null)
}
```

**Step 4: Register all new routes**

Add to the Router in `main()`:

```rust
.route("/api/create_project", post(create_project))
.route("/api/delete_project", post(delete_project))
.route("/api/get_agents_md", post(get_agents_md))
.route("/api/update_agents_md", post(update_agents_md))
.route("/api/set_initial_prompt", post(set_initial_prompt))
.route("/api/check_claude_cli", post(check_claude_cli))
.route("/api/home_dir", post(home_dir))
.route("/api/save_onboarding_config", post(save_onboarding_config))
.route("/api/log_frontend_error", post(log_frontend_error))
```

**Step 5: Make `next_session_label`, `validate_project_name`, `render_agents_md`, `ensure_claude_md_symlink` public**

These functions in `commands.rs` are `pub(crate)` or `pub` but need to be accessible from the server binary via `the_controller_lib::commands::*`. Verify they're exported from the lib.

**Step 6: Build and verify**

```bash
cd src-tauri && cargo build --features server
cd src-tauri && cargo clippy --features server -- -D warnings
```

**Step 7: Commit**

```bash
git add src-tauri/src/bin/server.rs src-tauri/src/commands.rs
git commit -m "feat(server): wire create_session, create_project, and essential routes"
```

---

## Task 5: Add token-based authentication

On a headless server exposed to a network, the HTTP/WS endpoints need auth. A simple bearer token is sufficient.

**Files:**
- Modify: `src-tauri/src/bin/server.rs`

**Step 1: Add auth middleware**

```rust
use axum::{
    extract::Request,
    middleware::{self, Next},
    response::Response,
};

async fn auth_middleware(req: Request, next: Next) -> Result<Response, StatusCode> {
    let token = match std::env::var("CONTROLLER_AUTH_TOKEN") {
        Ok(t) if !t.is_empty() => t,
        _ => return Ok(next.run(req).await), // No token configured = no auth
    };

    // Skip auth for static files (non-API, non-WS)
    let path = req.uri().path();
    if !path.starts_with("/api/") && path != "/ws" {
        return Ok(next.run(req).await);
    }

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
            .and_then(|q| q.split('&').find_map(|p| p.strip_prefix("token=")))
            .map(|t| t == token)
            .unwrap_or(false);

    if authorized {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}
```

Apply it to the router:

```rust
let app = Router::new()
    // ... routes ...
    .layer(middleware::from_fn(auth_middleware))
    .layer(CorsLayer::permissive())
    .with_state(state);
```

**Step 2: Update frontend `backend.ts` to send auth token**

```typescript
function getAuthToken(): string | null {
  if (isTauri) return null;
  const params = new URLSearchParams(window.location.search);
  return params.get("token") || null;
}

const authToken = getAuthToken();

export async function command<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(cmd, args);
  }
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  if (authToken) headers["Authorization"] = `Bearer ${authToken}`;
  const res = await fetch(`/api/${cmd}`, {
    method: "POST",
    headers,
    body: JSON.stringify(args ?? {}),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}
```

And for WebSocket:

```typescript
function getSharedWebSocket(): WebSocket {
  if (!sharedWs || sharedWs.readyState === WebSocket.CLOSED || sharedWs.readyState === WebSocket.CLOSING) {
    const tokenParam = authToken ? `?token=${authToken}` : "";
    const wsUrl = `ws://${window.location.hostname}:${window.location.port || 3001}/ws${tokenParam}`;
    sharedWs = new WebSocket(wsUrl);
  }
  return sharedWs;
}
```

**Step 3: Print auth URL on startup**

```rust
// In main():
let token = std::env::var("CONTROLLER_AUTH_TOKEN").ok();
match &token {
    Some(t) => println!("Server listening on http://0.0.0.0:{}?token={}", port, t),
    None => println!("Server listening on http://0.0.0.0:{} (no auth)", port),
}
```

**Step 4: Commit**

```bash
git add src-tauri/src/bin/server.rs src/lib/backend.ts
git commit -m "feat(server): add bearer token authentication"
```

---

## Task 6: Add CLI argument parsing and graceful shutdown

**Files:**
- Modify: `src-tauri/src/bin/server.rs`

**Step 1: Add CLI args with environment variable fallbacks**

```rust
fn get_port() -> u16 {
    std::env::var("CONTROLLER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3001)
}

fn get_bind_address() -> String {
    std::env::var("CONTROLLER_BIND")
        .unwrap_or_else(|_| "0.0.0.0".to_string())
}
```

**Step 2: Add graceful shutdown with tmux cleanup**

```rust
use tokio::signal;

async fn shutdown_signal(app_state: Arc<ServerState>) {
    let ctrl_c = async { signal::ctrl_c().await.unwrap() };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
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

    println!("\nShutting down...");

    // Clean up status socket
    the_controller_lib::status_socket::cleanup();

    // Kill all PTY/tmux sessions
    if let Ok(mut pty_manager) = app_state.app.pty_manager.lock() {
        let ids = pty_manager.session_ids();
        for id in ids {
            let _ = pty_manager.close_session(id);
        }
    }
}

// In main():
let shutdown_state = state.clone();
axum::serve(listener, app)
    .with_graceful_shutdown(shutdown_signal(shutdown_state))
    .await
    .unwrap();
```

**Step 3: Start status socket in server**

```rust
// In main(), after creating app_state:
let state_for_socket = Arc::new(app_state.clone()); // or restructure
the_controller_lib::status_socket::start_listener_with_state(state_for_socket);
```

This depends on Task 1 being complete.

**Step 4: Commit**

```bash
git add src-tauri/src/bin/server.rs
git commit -m "feat(server): add CLI config and graceful shutdown"
```

---

## Task 7: Wire remaining secondary routes

These are needed for full feature parity but not for basic operation.

**Files:**
- Modify: `src-tauri/src/bin/server.rs`

**Routes to wire (pattern: read from storage, return JSON):**

| Route | Source in commands.rs | Complexity |
|-------|----------------------|------------|
| `scaffold_project` | L371-333 | Medium (shells out to `gh`) |
| `list_github_issues` | L1415 | Medium (HTTP to GitHub) |
| `list_assigned_issues` | L1423 | Medium |
| `create_github_issue` | L1435 | Medium |
| `post_github_comment` | L1445 | Low |
| `add_github_label` | L1454 | Low |
| `remove_github_label` | L1466 | Low |
| `get_session_commits` | L (uses git2) | Medium |
| `configure_maintainer` | L1851 | Low |
| `get_maintainer_status` | L1952 | Low |
| `get_maintainer_history` | L1964 | Low |
| `trigger_maintainer_check` | L2008 | Medium |
| `configure_auto_worker` | L1872 | Low |
| `get_auto_worker_queue` | L1934 | Low |
| `get_worker_reports` | L1888 | Low |
| `get_session_token_usage` | L (token_usage) | Low |
| `save_session_prompt` | L (storage) | Low |
| `list_project_prompts` | L (storage) | Low |
| `get_repo_head` | L (git2) | Low |
| `list_directories_at` | L (fs) | Low |
| `list_root_directories` | L (fs) | Low |

**Strategy:** For each route, the pattern is the same — extract args from JSON, call the same underlying lib function, return JSON. Many of these can be wired mechanically.

**Desktop-only routes to skip (return NOT_IMPLEMENTED in browser):**
- `copy_image_file_to_clipboard` — requires native clipboard
- `capture_app_screenshot` — requires Tauri window
- `start_voice_pipeline` / `stop_voice_pipeline` — requires audio hardware
- `load_terminal_theme` — Tauri-specific asset loading

**Step 1: Wire routes in batches, build after each batch**

```bash
cd src-tauri && cargo build --features server
cd src-tauri && cargo clippy --features server -- -D warnings
```

**Step 2: Commit per batch**

```bash
git commit -m "feat(server): wire GitHub issue routes"
git commit -m "feat(server): wire maintainer and auto-worker routes"
git commit -m "feat(server): wire remaining storage routes"
```

---

## Task 8: Fix WebSocket reconnection in browser mode

The current `getSharedWebSocket()` doesn't handle reconnection. If the WS drops, events stop flowing.

**Files:**
- Modify: `src/lib/backend.ts`

**Step 1: Add auto-reconnect with exponential backoff**

```typescript
let wsListeners: Array<(msg: MessageEvent) => void> = [];
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
let reconnectDelay = 1000;

function connectWebSocket(): WebSocket {
  const tokenParam = authToken ? `?token=${authToken}` : "";
  const wsUrl = `ws://${window.location.host}/ws${tokenParam}`;
  const ws = new WebSocket(wsUrl);

  ws.addEventListener("open", () => {
    reconnectDelay = 1000; // reset on success
  });

  ws.addEventListener("message", (msg) => {
    for (const listener of wsListeners) {
      listener(msg);
    }
  });

  ws.addEventListener("close", () => {
    if (reconnectTimer) return;
    reconnectTimer = setTimeout(() => {
      reconnectTimer = null;
      sharedWs = connectWebSocket();
    }, reconnectDelay);
    reconnectDelay = Math.min(reconnectDelay * 2, 30000);
  });

  return ws;
}

function getSharedWebSocket(): WebSocket {
  if (!sharedWs || sharedWs.readyState === WebSocket.CLOSED || sharedWs.readyState === WebSocket.CLOSING) {
    sharedWs = connectWebSocket();
  }
  return sharedWs;
}
```

Update `listen()` to use `wsListeners` array instead of adding directly to the WS instance:

```typescript
export function listen<T>(event: string, handler: (payload: T) => void): () => void {
  if (isTauri) { /* ... existing ... */ }

  getSharedWebSocket(); // ensure connected
  const callback = (msg: MessageEvent) => {
    const data = JSON.parse(msg.data);
    if (data.event === event) handler(data.payload);
  };
  wsListeners.push(callback);
  return () => {
    wsListeners = wsListeners.filter((l) => l !== callback);
  };
}
```

**Step 2: Test and commit**

```bash
pnpm test
git add src/lib/backend.ts
git commit -m "fix: add WebSocket auto-reconnect for browser mode"
```

---

## Task 9: End-to-end validation

**Step 1: Build everything**

```bash
pnpm build
cd src-tauri && cargo build --release --features server
```

**Step 2: Test browser mode**

```bash
CONTROLLER_DIST_DIR=../dist \
CONTROLLER_AUTH_TOKEN=test123 \
./src-tauri/target/release/server
```

Open `http://localhost:3001?token=test123` in browser. Verify:
- [ ] App loads (Svelte UI renders)
- [ ] Onboarding flow works
- [ ] Can create a project
- [ ] Can create a session (terminal appears)
- [ ] Can type in terminal (PTY I/O works)
- [ ] Session status updates (idle/working indicators)
- [ ] Can close a session
- [ ] WebSocket reconnects after disconnect

**Step 3: Test desktop mode still works**

```bash
pnpm tauri dev
```

Verify all existing functionality is unbroken.

**Step 4: Commit**

```bash
git commit -m "test: validate browser and desktop modes"
```

---

## Execution Order & Dependencies

```
Task 1 (decouple status_socket) ──┐
Task 2 (static file serving)  ────┤
Task 3 (guard Tauri imports)  ────┼── can run in parallel
Task 5 (auth)                 ────┤
Task 8 (WS reconnect)        ────┘
                                  │
Task 4 (wire critical routes) ────┤── depends on Task 1 for status socket
Task 6 (CLI + shutdown)      ────┘
                                  │
Task 7 (secondary routes)    ────── depends on Task 4 patterns
                                  │
Task 9 (E2E validation)      ────── depends on all above
```

Tasks 1, 2, 3, 5, 8 are independent and can be parallelized.
