# Playwright + Axum E2E Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Add a browser-compatible backend transport (Axum HTTP + WebSocket) and Playwright-driven e2e tests with video recording, starting with a merge-codex demo.

**Architecture:** Frontend command adapter (`src/lib/backend.ts`) detects runtime (Tauri vs browser) and routes commands to either `invoke()` or `fetch()`. Axum server binary exposes same commands as REST + WebSocket. Playwright drives Chromium against vite dev server + Axum backend, records video.

**Tech Stack:** Axum 0.8, tokio-tungstenite, Playwright, Svelte 5, Rust

---

### Task 1: Create Command Adapter

**Files:**
- Create: `src/lib/backend.ts`
- Test: `src/lib/backend.test.ts`

**Step 1: Write the failing test**

```typescript
// src/lib/backend.test.ts
import { describe, it, expect, vi, beforeEach } from "vitest";

// Mock the Tauri API
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("backend adapter", () => {
  beforeEach(() => {
    vi.resetModules();
    // Clear __TAURI__ for each test
    delete (window as any).__TAURI__;
  });

  it("should use invoke when __TAURI__ is present", async () => {
    (window as any).__TAURI__ = {};
    const { command } = await import("./backend");
    const { invoke } = await import("@tauri-apps/api/core");
    (invoke as any).mockResolvedValue({ id: "123" });

    const result = await command("list_projects");
    expect(invoke).toHaveBeenCalledWith("list_projects", undefined);
    expect(result).toEqual({ id: "123" });
  });

  it("should use fetch when __TAURI__ is absent", async () => {
    const mockResponse = { id: "456" };
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(mockResponse),
      text: () => Promise.resolve(""),
    });

    const { command } = await import("./backend");
    const result = await command("create_project", { name: "test" });

    expect(fetch).toHaveBeenCalledWith("/api/create_project", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name: "test" }),
    });
    expect(result).toEqual(mockResponse);
  });

  it("should throw on non-ok fetch response", async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: false,
      text: () => Promise.resolve("not found"),
    });

    const { command } = await import("./backend");
    await expect(command("bad_command")).rejects.toThrow("not found");
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/backend.test.ts`
Expected: FAIL — module `./backend` not found

**Step 3: Write the implementation**

```typescript
// src/lib/backend.ts

const isTauri = typeof window !== "undefined" && !!(window as any).__TAURI__;

let sharedWs: WebSocket | null = null;

function getSharedWebSocket(): WebSocket {
  if (!sharedWs || sharedWs.readyState === WebSocket.CLOSED) {
    const wsUrl = `ws://${window.location.hostname}:3001/ws`;
    sharedWs = new WebSocket(wsUrl);
  }
  return sharedWs;
}

export async function command<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(cmd, args);
  }
  const res = await fetch(`/api/${cmd}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(args ?? {}),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export function listen<T>(event: string, handler: (payload: T) => void): () => void {
  if (isTauri) {
    let unlisten: (() => void) | undefined;
    import("@tauri-apps/api/event").then(({ listen }) => {
      listen<T>(event, (e) => handler(e.payload)).then((fn) => {
        unlisten = fn;
      });
    });
    return () => unlisten?.();
  }

  const ws = getSharedWebSocket();
  const callback = (msg: MessageEvent) => {
    const data = JSON.parse(msg.data);
    if (data.event === event) handler(data.payload);
  };
  ws.addEventListener("message", callback);
  return () => ws.removeEventListener("message", callback);
}
```

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/backend.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add src/lib/backend.ts src/lib/backend.test.ts
git commit -m "feat: add command adapter with Tauri/HTTP+WS dual transport"
```

---

### Task 2: Replace Frontend invoke/listen Calls

**Files to modify (17 files):**
- `src/App.svelte:4` — remove `invoke` import, add `import { command, listen } from "./lib/backend"`
- `src/lib/Sidebar.svelte:3-4` — replace both imports
- `src/lib/Terminal.svelte:8-9` — replace both imports
- `src/lib/SummaryPane.svelte:3-4` — replace both imports
- `src/lib/project-listing.ts:1` — replace import
- `src/lib/NotesEditor.svelte:4` — replace import
- `src/lib/Onboarding.svelte:2` — replace import
- `src/lib/TriagePanel.svelte:4` — replace import
- `src/lib/AgentDashboard.svelte:4` — replace import
- `src/lib/HotkeyManager.svelte:4` — replace import
- `src/lib/DeleteProjectModal.svelte:2` — replace import
- `src/lib/NewProjectModal.svelte:2` — replace import
- `src/lib/IssuePickerModal.svelte:3` — replace import
- `src/lib/PromptPickerModal.svelte:3` — replace import
- `src/lib/FuzzyFinder.svelte:2` — replace import
- `src/lib/AssignedIssuesPanel.svelte:4` — replace import
- `src/lib/sidebar/NotesTree.svelte:2` — replace import

**The replacement is mechanical:**

1. Replace `import { invoke } from "@tauri-apps/api/core"` with `import { command } from "$lib/backend"`
2. Replace `import { listen, type UnlistenFn } from "@tauri-apps/api/event"` with `import { listen } from "$lib/backend"` (keep `type UnlistenFn` as a local type or remove if unused)
3. Replace all `invoke(` with `command(`
4. Replace all `invoke<Type>(` with `command<Type>(`
5. For `listen` calls: the Tauri `listen` returns `Promise<UnlistenFn>` while our adapter returns `() => void` synchronously. Update call sites accordingly:
   - Before: `const unlisten = await listen<T>("event", handler)`
   - After: `const unlisten = listen<T>("event", (payload) => handler(payload))`
   - Note: Tauri's `listen` wraps payload in `{ payload: T }`, our adapter unwraps it already

**Step 1: Do the replacements**

For each of the 17 files:
- Update the import line
- Replace `invoke(` → `command(` and `invoke<` → `command<`
- Update `listen()` call sites to match new signature (synchronous return, payload already unwrapped)

**Step 2: Update test mocks**

Files with test mocks that reference `@tauri-apps/api/core`:
- `src/lib/Sidebar.test.ts:3`
- `src/lib/project-listing.test.ts:2`
- `src/lib/NotesEditor.test.ts:3`
- `src/App.test.ts:3`
- `vitest-setup.ts` (mocks `@tauri-apps/api/core` and `@tauri-apps/api/event`)

Update these to mock `$lib/backend` instead:
```typescript
vi.mock("$lib/backend", () => ({
  command: vi.fn(),
  listen: vi.fn(() => () => {}),
}));
```

And update `vitest-setup.ts` to mock `$lib/backend` globally instead of the Tauri APIs.

**Step 3: Run all frontend tests**

Run: `npx vitest run`
Expected: All existing tests pass

**Step 4: Run the Tauri app in dev mode to verify it still works**

Run: `npm run tauri dev`
Expected: App works identically (adapter detects `__TAURI__` and uses `invoke()`)

**Step 5: Commit**

```bash
git add -A
git commit -m "refactor: replace direct invoke/listen with command adapter"
```

---

### Task 3: EventEmitter Trait in Rust

**Files:**
- Create: `src-tauri/src/emitter.rs`
- Modify: `src-tauri/src/lib.rs` — add `pub mod emitter`
- Modify: `src-tauri/src/state.rs` — add emitter to `AppState`
- Modify: `src-tauri/src/pty_manager.rs` — use trait instead of `AppHandle`
- Modify: `src-tauri/src/commands.rs` — use trait instead of `AppHandle`
- Modify: `src-tauri/src/status_socket.rs` — use trait
- Modify: `src-tauri/src/maintainer.rs` — use trait
- Modify: `src-tauri/src/auto_worker.rs` — use trait

**Step 1: Write the trait and Tauri implementation**

```rust
// src-tauri/src/emitter.rs
use std::sync::Arc;

pub trait EventEmitter: Send + Sync + 'static {
    fn emit(&self, event: &str, payload: &str) -> Result<(), String>;
}

/// Tauri implementation — wraps AppHandle.emit()
pub struct TauriEmitter {
    app_handle: tauri::AppHandle,
}

impl TauriEmitter {
    pub fn new(app_handle: tauri::AppHandle) -> Arc<dyn EventEmitter> {
        Arc::new(Self { app_handle })
    }
}

impl EventEmitter for TauriEmitter {
    fn emit(&self, event: &str, payload: &str) -> Result<(), String> {
        self.app_handle
            .emit(event, payload)
            .map_err(|e| e.to_string())
    }
}
```

**Step 2: Add emitter to AppState**

Modify `src-tauri/src/state.rs`:
```rust
pub struct AppState {
    pub storage: Mutex<Storage>,
    pub pty_manager: Arc<Mutex<PtyManager>>,
    pub issue_cache: Arc<Mutex<IssueCache>>,
    pub emitter: Arc<dyn crate::emitter::EventEmitter>,
}
```

Update `AppState::new()` to accept an emitter parameter, and update the Tauri setup in `lib.rs` to pass `TauriEmitter::new(app.handle().clone())`.

**Step 3: Replace all `app_handle.emit()` calls**

Across the 5 Rust files with `app_handle.emit()` (21 occurrences):
- `pty_manager.rs` (9 calls) — the read loops emit PTY output and status
- `commands.rs` (8 calls) — merge-status, staging-status, maintainer-status
- `status_socket.rs` (2 calls) — session status hook events
- `auto_worker.rs` (1 call) — auto-worker status
- `maintainer.rs` (1 call) — maintainer status

Replace `app_handle.emit(event, payload)` with `state.emitter.emit(event, &payload)` (or pass the emitter through to functions that need it).

**Step 4: Run Rust tests**

Run: `cd src-tauri && cargo test`
Expected: All tests pass

**Step 5: Run the Tauri app to verify**

Run: `npm run tauri dev`
Expected: App works identically

**Step 6: Commit**

```bash
git add src-tauri/
git commit -m "refactor: extract EventEmitter trait from app_handle.emit()"
```

---

### Task 4: Axum Server Binary

**Files:**
- Modify: `src-tauri/Cargo.toml` — add axum, tokio-tungstenite deps
- Create: `src-tauri/src/bin/server.rs`
- Create: `src-tauri/src/emitter.rs` (add WsEmitter)

**Step 1: Add dependencies to Cargo.toml**

Add to `[dependencies]`:
```toml
axum = { version = "0.8", optional = true }
tokio-tungstenite = { version = "0.26", optional = true }
tower-http = { version = "0.6", features = ["cors"], optional = true }

[features]
server = ["axum", "tokio-tungstenite", "tower-http"]

[[bin]]
name = "server"
path = "src/bin/server.rs"
required-features = ["server"]
```

**Step 2: Write the WsEmitter**

Add to `src-tauri/src/emitter.rs`:

```rust
use std::sync::Mutex;
use tokio::sync::broadcast;

/// WebSocket implementation — broadcasts events to connected clients.
pub struct WsBroadcastEmitter {
    tx: broadcast::Sender<String>,
}

impl WsBroadcastEmitter {
    pub fn new() -> (Arc<dyn EventEmitter>, broadcast::Receiver<String>) {
        let (tx, rx) = broadcast::channel(1024);
        (Arc::new(Self { tx }), rx)
    }

    pub fn subscribe(emitter: &Arc<dyn EventEmitter>) -> broadcast::Receiver<String> {
        // Downcast not needed — we keep the Sender accessible
        todo!("Use a shared Sender via state")
    }
}

impl EventEmitter for WsBroadcastEmitter {
    fn emit(&self, event: &str, payload: &str) -> Result<(), String> {
        let msg = serde_json::json!({ "event": event, "payload": payload }).to_string();
        let _ = self.tx.send(msg); // Ok if no receivers
        Ok(())
    }
}
```

**Step 3: Write the Axum server**

```rust
// src-tauri/src/bin/server.rs
use axum::{
    extract::State as AxumState,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use std::sync::Arc;
use the_controller_lib::{
    commands, emitter::WsBroadcastEmitter, state::AppState, storage::Storage,
};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

struct ServerState {
    app_state: AppState,
    ws_tx: broadcast::Sender<String>,
}

#[tokio::main]
async fn main() {
    let (emitter, _rx) = WsBroadcastEmitter::new();
    let app_state = AppState::with_emitter(emitter);

    let app = Router::new()
        // Core project commands
        .route("/api/list_projects", post(list_projects))
        .route("/api/create_project", post(create_project))
        .route("/api/load_project", post(load_project))
        // Session commands
        .route("/api/create_session", post(create_session))
        .route("/api/connect_session", post(connect_session))
        .route("/api/restore_sessions", post(restore_sessions))
        .route("/api/write_to_pty", post(write_to_pty))
        .route("/api/send_raw_to_pty", post(send_raw_to_pty))
        .route("/api/resize_pty", post(resize_pty))
        .route("/api/close_session", post(close_session))
        // Merge
        .route("/api/merge_session_branch", post(merge_session_branch))
        // Config
        .route("/api/check_onboarding", post(check_onboarding))
        // Catch-all for unimplemented commands
        .fallback(post(|| async { Json(serde_json::Value::Null) }))
        // WebSocket for events
        .route("/ws", axum::routing::get(ws_handler))
        .layer(CorsLayer::permissive())
        .with_state(Arc::new(app_state));

    println!("Axum server listening on http://localhost:3001");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// Each route handler deserializes JSON args and calls the existing command function.
// Example for list_projects:
async fn list_projects(
    AxumState(state): AxumState<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Reuse the existing command logic
    // Note: Tauri commands take State<AppState>, we need to adapt
    // The command functions need to be callable without Tauri's State wrapper.
    // This may require extracting the core logic from the #[tauri::command] functions.
    todo!("Wire up to existing command logic")
}
```

**Important implementation note:** The `#[tauri::command]` functions take `State<AppState>` (Tauri's wrapper) and `AppHandle`. For the Axum server, we need the core logic callable with plain `&AppState`. The cleanest approach is to extract the business logic from each command into a plain function, then have both the Tauri command and Axum handler call it:

```rust
// In commands.rs — extract core logic:
pub fn list_projects_core(state: &AppState) -> Result<ProjectInventory, String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    storage.list_projects().map_err(|e| e.to_string())
}

// Tauri command (unchanged interface):
#[tauri::command]
pub fn list_projects(state: State<AppState>) -> Result<ProjectInventory, String> {
    list_projects_core(&state)
}

// Axum handler:
async fn list_projects(AxumState(state): AxumState<Arc<AppState>>) -> ... {
    commands::list_projects_core(&state)
}
```

**Step 4: Implement the WebSocket handler for PTY output**

```rust
async fn ws_handler(
    ws: axum::extract::WebSocketUpgrade,
    AxumState(state): AxumState<Arc<ServerState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: axum::extract::ws::WebSocket, state: Arc<ServerState>) {
    let mut rx = state.ws_tx.subscribe();
    while let Ok(msg) = rx.recv().await {
        if socket.send(axum::extract::ws::Message::Text(msg.into())).await.is_err() {
            break;
        }
    }
}
```

**Step 5: Verify the server starts**

Run: `cd src-tauri && cargo run --bin server --features server`
Expected: "Axum server listening on http://localhost:3001"

Test with curl: `curl -X POST http://localhost:3001/api/list_projects`
Expected: JSON response with project list

**Step 6: Commit**

```bash
git add src-tauri/
git commit -m "feat: add Axum HTTP+WS server binary for browser-mode backend"
```

---

### Task 5: Vite Proxy + Playwright Setup

**Files:**
- Modify: `vite.config.ts` — add proxy for `/api` and `/ws` to Axum server
- Modify: `package.json` — add playwright deps and scripts
- Create: `playwright.config.ts`
- Create: `e2e/specs/smoke.spec.ts` (rewrite for Playwright)

**Step 1: Add Vite proxy**

Modify `vite.config.ts` to proxy API calls to Axum:

```typescript
server: {
  proxy: {
    "/api": "http://localhost:3001",
    "/ws": {
      target: "ws://localhost:3001",
      ws: true,
    },
  },
},
```

This means the Svelte frontend at `http://localhost:1420` can call `/api/list_projects` and it gets proxied to the Axum server. No CORS issues.

**Step 2: Install Playwright**

Run:
```bash
npm install --save-dev @playwright/test
npx playwright install chromium
```

**Step 3: Write Playwright config**

```typescript
// playwright.config.ts
import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e/specs",
  timeout: 300_000, // 5 minutes for slow Codex workflows
  use: {
    baseURL: "http://localhost:1420",
    video: "on",
  },
  webServer: [
    {
      command: "cd src-tauri && cargo run --bin server --features server",
      port: 3001,
      reuseExistingServer: true,
      timeout: 120_000,
    },
    {
      command: "npm run dev",
      port: 1420,
      reuseExistingServer: true,
    },
  ],
  projects: [
    {
      name: "e2e",
      use: { browserName: "chromium" },
    },
    {
      name: "demo",
      use: {
        browserName: "chromium",
        video: "on",
        viewport: { width: 1280, height: 800 },
      },
    },
  ],
  outputDir: "e2e/results",
});
```

**Step 4: Write smoke test**

```typescript
// e2e/specs/smoke.spec.ts
import { test, expect } from "@playwright/test";

test("app loads and renders sidebar", async ({ page }) => {
  await page.goto("/");
  await expect(page).toHaveTitle("The Controller");
  await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });
});
```

**Step 5: Add scripts to package.json**

```json
"test:e2e": "npx playwright test --project=e2e",
"demo": "npx playwright test --project=demo"
```

**Step 6: Run smoke test**

Run: `npm run test:e2e -- --grep smoke`
Expected: Passes, video recorded in `e2e/results/`

**Step 7: Commit**

```bash
git add playwright.config.ts vite.config.ts package.json package-lock.json e2e/specs/smoke.spec.ts
git commit -m "feat: add Playwright e2e setup with vite proxy and smoke test"
```

---

### Task 6: Merge-Codex Demo Spec

**Files:**
- Modify: `e2e/specs/merge-codex.spec.ts` (rewrite for Playwright)
- Reuse: `e2e/helpers/repo-setup.ts`, `e2e/helpers/project-seed.ts`

**Step 1: Rewrite the merge-codex spec for Playwright**

```typescript
// e2e/specs/merge-codex.spec.ts
import { test, expect } from "@playwright/test";
import { setupTestRepo, cleanupTestRepo, SANDBOX_REPO, type TestRepo } from "../helpers/repo-setup";
import { seedProject, cleanupSeededProject, type SeededProject } from "../helpers/project-seed";
import { execSync } from "node:child_process";

let repo: TestRepo;
let seeded: SeededProject;

test.beforeAll(() => {
  repo = setupTestRepo();
  seeded = seedProject(repo.localPath, repo.branchName);
});

test.afterAll(() => {
  if (repo) cleanupTestRepo(repo);
  if (seeded) cleanupSeededProject(seeded);
});

test("merge codex session branch creates a PR", async ({ page }) => {
  await page.goto("/");

  // Wait for sidebar to render with our seeded project
  const sessionEl = page.locator(`.session-label`, { hasText: repo.branchName });
  await expect(sessionEl).toBeVisible({ timeout: 15_000 });

  // Click to focus the session
  await sessionEl.click();

  // Press 'm' to trigger finish-branch
  await page.keyboard.press("m");

  // ConfirmModal should appear
  await expect(page.locator(".modal-header", { hasText: "Confirm Merge" })).toBeVisible({
    timeout: 5_000,
  });

  // Press 'y' to confirm
  await page.keyboard.press("y");

  // Poll GitHub for the PR
  let prUrl = "";
  const maxWaitMs = 180_000;
  const pollIntervalMs = 5_000;
  const startTime = Date.now();

  while (Date.now() - startTime < maxWaitMs) {
    try {
      prUrl = execSync(
        `gh pr view ${repo.branchName} --repo ${SANDBOX_REPO} --json url -q .url`,
        { encoding: "utf-8" },
      ).trim();
      if (prUrl) break;
    } catch {
      // Not yet
    }
    await page.waitForTimeout(pollIntervalMs);
  }

  expect(prUrl).toMatch(/github\.com/);
});
```

**Step 2: Run the demo**

Run: `npm run demo -- --grep merge`
Expected: Test passes, `.webm` video of the entire flow saved to `e2e/results/`

**Step 3: Commit**

```bash
git add e2e/specs/merge-codex.spec.ts
git commit -m "feat: add merge-codex Playwright demo spec with video recording"
```

---

### Task 7: Clean Up WebdriverIO Artifacts

**Files:**
- Delete: `wdio.conf.ts`
- Delete: `tsconfig.e2e.json`
- Modify: `package.json` — remove `@wdio/*`, `webdriverio`, `ts-node` devDependencies
- Modify: `.gitignore` — replace wdio entries with playwright entries

**Step 1: Remove wdio files and deps**

```bash
rm wdio.conf.ts tsconfig.e2e.json
npm uninstall @wdio/cli @wdio/local-runner @wdio/mocha-framework @wdio/spec-reporter webdriverio ts-node
```

**Step 2: Update .gitignore**

Replace:
```
# e2e test artifacts
wdio-logs/
e2e/screenshots/
```
With:
```
# e2e test artifacts
e2e/results/
test-results/
playwright-report/
```

**Step 3: Commit**

```bash
git add -A
git commit -m "chore: remove WebdriverIO, replace with Playwright"
```

---

## Future Work (Not In This Plan)

**Remaining command wiring:** Only ~10 commands are wired in Task 4. The remaining ~50 commands need `_core()` extractions and Axum route handlers. The catch-all fallback returns null for these, so the app loads but some features won't work in browser mode. Add commands incrementally as needed for new demo specs.

**Commands not yet wired:**
- Project management: `archive_project`, `delete_project`, `unarchive_project`, `scaffold_project`, etc.
- GitHub: `list_github_issues`, `create_github_issue`, `add_github_label`, etc.
- Notes: `list_notes`, `read_note`, `write_note`, `create_note`, etc.
- Maintainer: `configure_maintainer`, `trigger_maintainer_check`, etc.
- Auto-worker: `configure_auto_worker`, `get_worker_reports`
- Media: `copy_image_file_to_clipboard`, `capture_app_screenshot`
- Config: `save_onboarding_config`, `list_directories_at`, etc.
