# Controller Agent Architecture Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Implement the approved agent architecture so the Controller browser talks only to the Controller backend, the daemon owns durable agent/chat/profile/turn state, and chat routing, agent creation, and observability can ship incrementally.

**Architecture:** `the-controller` remains the browser-facing Axum + Svelte workspace owner. `the-controller-daemon` becomes the long-lived local agent owner behind a private Unix domain socket, with SQLite-backed profiles, chats, routing links, sessions, events, turns, and metrics. The browser calls same-origin `/api/daemon/...` routes only; the Controller backend forwards to the daemon over UDS and sends Controller-owned workspace snapshots when workspaces change.

**Tech Stack:** Rust, axum, tokio, SQLite/sqlx in `the-controller-daemon`; Rust, axum, reqwest/hyper UDS gateway in `the-controller`; Svelte 5 runes, Vitest, Playwright.

---

## Source Context

Read before implementation:

- `docs/domain-knowledge.md`
- `docs/plans/2026-04-29-agent-architecture-design.md`
- `docs/plans/2026-04-29-chat-routing-prd.md`
- `docs/plans/2026-04-29-agent-creation-prd.md`
- `docs/plans/2026-04-29-agent-observability-prd.md`
- `docs/plans/2026-04-29-controller-agent-product-prd.md`
- `/Users/noelkwan/projects/the-controller-daemon/docs/plans/2026-04-20-daemon-chat-architecture-design.md`
- `/Users/noelkwan/projects/the-controller-daemon/docs/plans/2026-04-26-adhoc-agent-profiles-design.md`

This is a two-repository implementation:

- Controller repo: `/Users/noelkwan/.codex/worktrees/7156/the-controller`
- Daemon repo: `/Users/noelkwan/projects/the-controller-daemon`

No old-data migration is required by the architecture, but existing tests use current migrations. Prefer additive migrations while tests still exist; delete or rewrite old assumptions only when the relevant replacement test exists and fails first.

## Required Execution Rules

- Use TDD for every behavior change.
- Before each production edit, write the failing test and run it.
- Do not expose daemon socket path, token, port, or transport details to browser JavaScript.
- Do not add a daemon TCP fallback in the target architecture.
- Do not let the daemon create, delete, rename, or migrate Controller worktrees.
- Offload blocking Controller request-handler work with `tokio::task::spawn_blocking`.
- Commit after each task or after the smallest coherent green slice if a task spans both repositories.
- If a task says to edit the daemon repo, run commands from `/Users/noelkwan/projects/the-controller-daemon`.
- If a task says to edit the Controller repo, run commands from `/Users/noelkwan/.codex/worktrees/7156/the-controller`.

## Baseline Setup

### Task 1: Prepare Branches And Baseline Verification

**Files:**
- Modify: no files expected unless dependency installation changes lockfiles.

**Step 1: Confirm Controller branch**

Run from the Controller repo:

```bash
git status --short --branch
```

Expected: on a non-main branch such as `codex/agent-implementation-plan` with no unexpected changes except this plan file if already saved.

**Step 2: Create daemon branch**

Run from the daemon repo:

```bash
git status --short --branch
git switch -c codex/agent-architecture-implementation
```

Expected: a new daemon branch. If the branch already exists, switch to it and verify it is not `main`.

**Step 3: Run Controller baseline tests**

Run from the Controller repo:

```bash
pnpm test
cd server && cargo test
```

Expected: the current baseline result is known before implementation. If either command fails, stop and report the failure instead of continuing.

**Step 4: Run daemon baseline tests**

Run from the daemon repo:

```bash
cargo test
```

Expected: the current daemon baseline result is known before implementation. If it fails, stop and report the failure instead of continuing.

**Step 5: Commit only if setup changed files**

If no files changed, do not commit. If lockfiles changed because dependencies were installed, inspect the diff and commit:

```bash
git diff --stat
git add <changed lockfiles>
git commit -m "chore: refresh implementation baseline"
```

## Transport And Gateway

### Task 2: Move Daemon Config To Unix Socket

**Files:**
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/config.rs`
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/main.rs`
- Create: `/Users/noelkwan/projects/the-controller-daemon/tests/uds_config.rs`
- Test: `/Users/noelkwan/projects/the-controller-daemon/tests/uds_config.rs`

**Step 1: Write the failing config test**

Create `tests/uds_config.rs`:

```rust
use std::path::PathBuf;
use the_controller_daemon::config::Config;

#[test]
fn default_config_uses_unix_socket_and_no_tcp_bind() {
    std::env::remove_var("TCD_STATE_DIR");
    std::env::remove_var("TCD_BIND");
    std::env::remove_var("TCD_SOCKET");

    let cfg = Config::default_from_home();

    assert_eq!(cfg.socket_path, PathBuf::from(std::env::var("HOME").unwrap()).join(".the-controller/daemon.sock"));
    assert!(!format!("{cfg:?}").contains("127.0.0.1:4867"));
}

#[test]
fn state_dir_overrides_socket_path() {
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("TCD_STATE_DIR", tmp.path());
    std::env::remove_var("TCD_SOCKET");

    let cfg = Config::from_env();

    assert_eq!(cfg.socket_path, tmp.path().join("daemon.sock"));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test uds_config
```

Expected: FAIL because `Config` still has `bind_addr` and no `socket_path`.

**Step 3: Implement minimal config change**

In `src/config.rs`, replace `bind_addr` with `socket_path`:

```rust
#[derive(Debug, Clone)]
pub struct Config {
    pub state_dir: PathBuf,
    pub db_path: PathBuf,
    pub token_path: PathBuf,
    pub pid_path: PathBuf,
    pub socket_path: PathBuf,
}
```

`default_from_home()` sets `state_dir.join("daemon.sock")`. `from_env()` supports `TCD_SOCKET` for tests and local debugging only. Remove `TCD_BIND` support.

**Step 4: Update main to bind UDS**

In `src/main.rs`, replace `TcpListener::bind(&cfg.bind_addr)` with a Unix listener. Before binding, remove a stale socket file only when no live daemon owns the pid lock.

Use a helper so it can be unit tested later:

```rust
#[cfg(unix)]
fn remove_stale_socket(path: &std::path::Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}
```

If `axum::serve` cannot serve `tokio::net::UnixListener` directly, add a local listener wrapper or a small `serve_unix` helper using hyper. Keep that helper in `src/main.rs` until a second call site exists.

**Step 5: Run transport tests**

Run:

```bash
cargo test --test uds_config
cargo test
```

Expected: PASS.

**Step 6: Commit**

Run:

```bash
git add src/config.rs src/main.rs tests/uds_config.rs
git commit -m "feat: bind daemon to unix socket"
```

### Task 3: Add Controller Daemon Gateway Client

**Files:**
- Modify: `server/Cargo.toml`
- Modify: `server/src/lib.rs`
- Create: `server/src/daemon_gateway.rs`
- Modify: `server/src/main.rs`
- Create: `server/tests/daemon_gateway.rs`
- Test: `server/tests/daemon_gateway.rs`

**Step 1: Write the failing gateway test**

Create `server/tests/daemon_gateway.rs` with a unit-level test for request construction. Keep it independent of a real daemon:

```rust
use the_controller_lib::daemon_gateway::{DaemonGatewayConfig, daemon_socket_path};

#[test]
fn daemon_socket_path_uses_controller_state_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = DaemonGatewayConfig {
        state_dir: tmp.path().to_path_buf(),
    };

    assert_eq!(daemon_socket_path(&cfg), tmp.path().join("daemon.sock"));
}
```

Add a second test once the gateway request builder exists:

```rust
#[test]
fn gateway_paths_must_stay_under_api_daemon() {
    assert!(the_controller_lib::daemon_gateway::normalize_daemon_path("/api/daemon/profiles").is_ok());
    assert!(the_controller_lib::daemon_gateway::normalize_daemon_path("/api/list_projects").is_err());
    assert!(the_controller_lib::daemon_gateway::normalize_daemon_path("/api/daemon/../projects").is_err());
}
```

**Step 2: Run test to verify it fails**

Run from the Controller repo:

```bash
cd server && cargo test --test daemon_gateway
```

Expected: FAIL because `daemon_gateway` does not exist.

**Step 3: Implement minimal gateway module**

Create `server/src/daemon_gateway.rs`:

```rust
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DaemonGatewayConfig {
    pub state_dir: PathBuf,
}

pub fn daemon_socket_path(cfg: &DaemonGatewayConfig) -> PathBuf {
    cfg.state_dir.join("daemon.sock")
}

pub fn normalize_daemon_path(path: &str) -> Result<String, String> {
    let rest = path
        .strip_prefix("/api/daemon")
        .ok_or_else(|| "path must start with /api/daemon".to_string())?;
    if rest.contains("..") {
        return Err("daemon gateway path must not contain ..".to_string());
    }
    Ok(if rest.is_empty() { "/".to_string() } else { rest.to_string() })
}
```

Export it from `server/src/lib.rs`:

```rust
pub mod daemon_gateway;
```

**Step 4: Add same-origin HTTP route skeleton**

In `server/src/main.rs`, add a gateway route under `/api/daemon/{*path}`. Start with `GET /api/daemon/health` if wildcard routing is awkward in axum 0.8, then generalize in the next task.

The handler must read the socket path from `state.app.storage.base_dir()` and must not read or return `daemon.token`.

**Step 5: Run tests**

Run:

```bash
cd server && cargo test --test daemon_gateway
cd server && cargo test
```

Expected: PASS.

**Step 6: Commit**

Run:

```bash
git add server/Cargo.toml server/src/lib.rs server/src/daemon_gateway.rs server/src/main.rs server/tests/daemon_gateway.rs
git commit -m "feat: add daemon gateway path handling"
```

### Task 4: Route Frontend Daemon Calls Through Same-Origin API

**Files:**
- Modify: `src/lib/daemon/client.ts`
- Modify: `src/lib/daemon/store.svelte.ts`
- Modify: `src/lib/daemon/stream.ts`
- Modify: `src/lib/daemon/client.test.ts`
- Modify: `src/lib/daemon/store.test.ts`
- Modify: `src/lib/daemon/stream.test.ts`

**Step 1: Write failing frontend client test**

In `src/lib/daemon/client.test.ts`, replace the token-header expectation with:

```ts
it("calls same-origin /api/daemon routes without exposing auth", async () => {
  const fetchMock = mockFetch([{ status: 200, body: [] }]);
  vi.stubGlobal("fetch", fetchMock);

  const c = new DaemonClient("/api/daemon");
  await c.listSessions();

  const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
  expect(url).toBe("/api/daemon/sessions");
  expect(init.headers).not.toMatchObject({ Authorization: expect.any(String) });
});
```

Add:

```ts
it("builds same-origin websocket URLs", () => {
  const c = new DaemonClient("/api/daemon");
  expect(c.wsUrl("s1", 5)).toBe("/api/daemon/sessions/s1/stream?since=5");
});
```

**Step 2: Run test to verify it fails**

Run:

```bash
pnpm test src/lib/daemon/client.test.ts
```

Expected: FAIL because `DaemonClient` requires a token and direct base URL.

**Step 3: Implement minimal client change**

Change `DaemonClient` constructor to:

```ts
export class DaemonClient {
  constructor(private baseUrl = "/api/daemon") {}
}
```

Remove `Authorization` header construction and remove `bearer`.

Change `wsUrl()` so same-origin relative HTTP routes become relative WS gateway routes. The stream helper can pass the relative URL directly to `new WebSocket(url)`.

**Step 4: Remove token bootstrap from store**

In `src/lib/daemon/store.svelte.ts`:

- Delete `token` from `StoreState`.
- Delete `read_daemon_token`.
- Construct `new DaemonClient("/api/daemon")`.
- Keep `pingDaemon()` using `listSessions()` or `/health`.

**Step 5: Run frontend tests**

Run:

```bash
pnpm test src/lib/daemon/client.test.ts src/lib/daemon/store.test.ts src/lib/daemon/stream.test.ts
pnpm test
```

Expected: PASS.

**Step 6: Commit**

Run:

```bash
git add src/lib/daemon/client.ts src/lib/daemon/store.svelte.ts src/lib/daemon/stream.ts src/lib/daemon/*.test.ts
git commit -m "feat: route daemon client through controller gateway"
```

## Durable Daemon Model

### Task 5: Add Profile Versions, Handles, Archive State, And Avatar Metadata

**Files:**
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/model.rs`
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/store/agents.rs`
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/api/agents.rs`
- Create: `/Users/noelkwan/projects/the-controller-daemon/migrations/0003_agent_architecture_profiles.sql`
- Create: `/Users/noelkwan/projects/the-controller-daemon/tests/store_profile_versions.rs`
- Create: `/Users/noelkwan/projects/the-controller-daemon/tests/http_profiles.rs`

**Step 1: Write failing store test**

Create `tests/store_profile_versions.rs`:

```rust
use tempfile::TempDir;
use the_controller_daemon::{db, model::*, store::{agents::AgentProfileStore, skills::SkillStore}};

#[tokio::test]
async fn every_profile_save_creates_immutable_version() {
    let tmp = TempDir::new().unwrap();
    let pool = db::open(&tmp.path().join("t.db")).await.unwrap();
    let agents = AgentProfileStore::new(pool);

    let created = agents.save_profile(NewAgentProfile {
        id: None,
        handle: "reviewer".into(),
        name: "Reviewer".into(),
        description: "Reviews code".into(),
        runtime: Agent::Codex,
        model: Some("gpt-5.3-codex".into()),
        skills: vec![],
        prompt: "Review carefully.".into(),
        default_workspace_behavior: "focused".into(),
        outbox_instructions: "Publish concise review notes.".into(),
    }).await.unwrap();

    let updated = agents.save_profile(NewAgentProfile {
        id: Some(created.profile.id.clone()),
        handle: "reviewer".into(),
        name: "Reviewer".into(),
        description: "Reviews code".into(),
        runtime: Agent::Codex,
        model: Some("gpt-5.3-codex".into()),
        skills: vec![],
        prompt: "Review even more carefully.".into(),
        default_workspace_behavior: "focused".into(),
        outbox_instructions: "Publish concise review notes.".into(),
    }).await.unwrap();

    assert_ne!(created.version.id, updated.version.id);
    assert_eq!(agents.versions_for_profile(&created.profile.id).await.unwrap().len(), 2);
    assert_eq!(agents.get_version(&created.version.id).await.unwrap().unwrap().prompt, "Review carefully.");
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test store_profile_versions
```

Expected: FAIL because the store has mutable profiles and no versions.

**Step 3: Add schema**

Create `migrations/0003_agent_architecture_profiles.sql` with tables or columns for:

```sql
ALTER TABLE agent_profiles ADD COLUMN handle TEXT;
ALTER TABLE agent_profiles ADD COLUMN archived_at INTEGER;
ALTER TABLE agent_profiles ADD COLUMN avatar_asset_path TEXT;
ALTER TABLE agent_profiles ADD COLUMN avatar_status TEXT NOT NULL DEFAULT 'initials';
ALTER TABLE agent_profiles ADD COLUMN avatar_error TEXT;

CREATE UNIQUE INDEX agent_profiles_active_handle
ON agent_profiles(handle)
WHERE archived_at IS NULL;

CREATE TABLE agent_profile_versions (
  id TEXT PRIMARY KEY,
  profile_id TEXT NOT NULL REFERENCES agent_profiles(id),
  runtime TEXT NOT NULL CHECK (runtime IN ('codex')),
  model TEXT,
  prompt TEXT NOT NULL,
  default_workspace_behavior TEXT NOT NULL,
  outbox_instructions TEXT NOT NULL,
  validation_result TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE agent_profile_version_skills (
  version_id TEXT NOT NULL REFERENCES agent_profile_versions(id) ON DELETE CASCADE,
  skill_id TEXT NOT NULL,
  position INTEGER NOT NULL CHECK (position >= 0),
  PRIMARY KEY (version_id, skill_id),
  UNIQUE (version_id, position),
  FOREIGN KEY (skill_id) REFERENCES skill_library(id)
);
```

If SQLite cannot add a `NOT NULL` column without a default for existing tables, use defaults in the migration and backfill in Rust tests as needed.

**Step 4: Update Rust model**

Add:

```rust
pub struct AgentProfileVersion {
    pub id: String,
    pub profile_id: String,
    pub runtime: Agent,
    pub model: Option<String>,
    pub prompt: String,
    pub skills: Vec<String>,
    pub default_workspace_behavior: String,
    pub outbox_instructions: String,
    pub validation_result: serde_json::Value,
    pub created_at: i64,
}

pub struct SavedAgentProfile {
    pub profile: AgentProfile,
    pub version: AgentProfileVersion,
}
```

Change `AgentProfile` to include `handle`, archive fields, avatar fields, and `active_version_id`.

**Step 5: Implement store APIs**

In `src/store/agents.rs`, add:

- `save_profile(NewAgentProfile) -> Result<SavedAgentProfile>`
- `archive(id) -> Result<AgentProfile>`
- `restore(id) -> Result<AgentProfile>`
- `versions_for_profile(profile_id) -> Result<Vec<AgentProfileVersion>>`
- `get_version(version_id) -> Result<Option<AgentProfileVersion>>`
- handle validation allowing lowercase letters, numbers, and hyphens.

Keep old `create/update` only as wrappers if existing tests still need them.

**Step 6: Add HTTP tests**

Create `tests/http_profiles.rs` to prove:

- `POST /profiles` creates profile and version.
- `PATCH /profiles/:id` creates a new version.
- `POST /profiles/:id/archive` hides it from active suggestions.
- `POST /profiles/:id/restore` restores it.
- handle collision among active profiles returns 422.

**Step 7: Implement profile API routes**

Create or adapt `src/api/agents.rs` so product routes are:

```text
GET    /profiles
POST   /profiles
GET    /profiles/:id
PATCH  /profiles/:id
POST   /profiles/:id/archive
POST   /profiles/:id/restore
POST   /profiles/:id/test-chat
```

Keep `/agents` aliases only if required by existing tests; mark them internal and remove after frontend migration.

**Step 8: Run tests**

Run:

```bash
cargo test --test store_profile_versions
cargo test --test http_profiles
cargo test
```

Expected: PASS.

**Step 9: Commit**

Run from the daemon repo:

```bash
git add src/model.rs src/store/agents.rs src/api/agents.rs migrations/0003_agent_architecture_profiles.sql tests/store_profile_versions.rs tests/http_profiles.rs
git commit -m "feat: version agent profiles"
```

### Task 6: Add Chats, Links, Workspace Snapshots, And Session Ownership

**Files:**
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/model.rs`
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/store/mod.rs`
- Create: `/Users/noelkwan/projects/the-controller-daemon/src/store/chats.rs`
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/store/sessions.rs`
- Create: `/Users/noelkwan/projects/the-controller-daemon/migrations/0004_chats_and_routes.sql`
- Create: `/Users/noelkwan/projects/the-controller-daemon/tests/store_chats.rs`

**Step 1: Write failing chat store test**

Create `tests/store_chats.rs`:

```rust
use tempfile::TempDir;
use the_controller_daemon::{db, model::*, store::chats::ChatStore};

#[tokio::test]
async fn creates_chat_and_workspace_snapshot_without_owning_workspace() {
    let tmp = TempDir::new().unwrap();
    let pool = db::open(&tmp.path().join("t.db")).await.unwrap();
    let chats = ChatStore::new(pool);

    let chat = chats.create_chat(NewChat {
        project_id: "controller-project".into(),
        title: "Review branch".into(),
    }).await.unwrap();

    let link = chats.add_workspace_link(&chat.id, NewChatWorkspaceLink {
        project_id: "controller-project".into(),
        workspace_id: "controller-workspace".into(),
        path: "/tmp/worktree".into(),
        label: "controller-chat-routing".into(),
        branch: Some("codex/chat-routing".into()),
        focused: true,
    }).await.unwrap();

    assert_eq!(link.path, "/tmp/worktree");
    assert!(link.focused);
    assert_eq!(chats.workspace_links(&chat.id).await.unwrap().len(), 1);
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test store_chats
```

Expected: FAIL because `ChatStore` does not exist.

**Step 3: Add schema**

Create `migrations/0004_chats_and_routes.sql`:

```sql
CREATE TABLE chats (
  id TEXT PRIMARY KEY,
  controller_project_id TEXT NOT NULL,
  title TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  deleted_at INTEGER
);

CREATE TABLE chat_messages (
  id TEXT PRIMARY KEY,
  chat_id TEXT NOT NULL REFERENCES chats(id),
  idempotency_id TEXT,
  body TEXT NOT NULL,
  token_spans TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE(chat_id, idempotency_id)
);

CREATE TABLE chat_agent_links (
  id TEXT PRIMARY KEY,
  chat_id TEXT NOT NULL REFERENCES chats(id),
  session_id TEXT NOT NULL REFERENCES sessions(id),
  profile_id TEXT NOT NULL REFERENCES agent_profiles(id),
  profile_version_id TEXT NOT NULL REFERENCES agent_profile_versions(id),
  route_type TEXT NOT NULL CHECK(route_type IN ('reusable','shadow')),
  focused INTEGER NOT NULL DEFAULT 0,
  token_source TEXT NOT NULL CHECK(token_source IN ('@agent','%agent','api')),
  created_at INTEGER NOT NULL
);

CREATE TABLE chat_workspace_links (
  id TEXT PRIMARY KEY,
  chat_id TEXT NOT NULL REFERENCES chats(id),
  controller_project_id TEXT NOT NULL,
  controller_workspace_id TEXT NOT NULL,
  path TEXT NOT NULL,
  label TEXT NOT NULL,
  branch TEXT,
  focused INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

ALTER TABLE sessions ADD COLUMN session_kind TEXT NOT NULL DEFAULT 'raw';
ALTER TABLE sessions ADD COLUMN owner_chat_id TEXT REFERENCES chats(id);
ALTER TABLE sessions ADD COLUMN profile_version_id TEXT REFERENCES agent_profile_versions(id);
ALTER TABLE sessions ADD COLUMN launch_context_snapshot TEXT;
```

**Step 4: Implement models and store**

Add model structs:

- `Chat`
- `NewChat`
- `ChatMessage`
- `NewChatMessage`
- `ChatAgentLink`
- `NewChatAgentLink`
- `ChatWorkspaceLink`
- `NewChatWorkspaceLink`
- `SessionKind`

Implement `ChatStore` with methods:

- `create_chat`
- `get_chat`
- `list_chats`
- `mark_deleted`
- `append_message`
- `add_agent_link`
- `agent_links`
- `add_workspace_link`
- `workspace_links`
- `set_workspace_focus`

**Step 5: Update SessionStore**

Update `NewSession` and `Session` with:

- `session_kind`
- `owner_chat_id`
- `profile_version_id`
- `launch_context_snapshot`

Do not let `SessionStore` infer workspace paths. It stores only daemon-owned launch metadata.

**Step 6: Run tests**

Run:

```bash
cargo test --test store_chats
cargo test
```

Expected: PASS.

**Step 7: Commit**

Run:

```bash
git add src/model.rs src/store/mod.rs src/store/chats.rs src/store/sessions.rs migrations/0004_chats_and_routes.sql tests/store_chats.rs
git commit -m "feat: store chats and routing links"
```

### Task 7: Add Turns, Turn Events, And Metrics Store

**Files:**
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/model.rs`
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/store/mod.rs`
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/store/events.rs`
- Create: `/Users/noelkwan/projects/the-controller-daemon/src/store/turns.rs`
- Create: `/Users/noelkwan/projects/the-controller-daemon/migrations/0005_turns_and_metrics.sql`
- Create: `/Users/noelkwan/projects/the-controller-daemon/tests/store_turns.rs`

**Step 1: Write failing turns test**

Create `tests/store_turns.rs`:

```rust
use tempfile::TempDir;
use the_controller_daemon::{db, model::*, store::{events::EventStore, sessions::SessionStore, turns::TurnStore}};

#[tokio::test]
async fn turn_starts_at_inbox_and_events_attach_to_turn() {
    let tmp = TempDir::new().unwrap();
    let pool = db::open(&tmp.path().join("t.db")).await.unwrap();
    let sessions = SessionStore::new(pool.clone());
    let events = EventStore::new(pool.clone());
    let turns = TurnStore::new(pool);

    let session = sessions.create(NewSession {
        id: "s1".into(),
        label: "reviewer".into(),
        agent: Agent::Codex,
        agent_profile_id: None,
        profile_version_id: None,
        session_kind: SessionKind::Reusable,
        owner_chat_id: None,
        cwd: "/tmp".into(),
        args: vec![],
        launch_context_snapshot: None,
    }).await.unwrap();

    let inbox = events.append_for_chat(&session.id, Some("chat1"), None, NewEvent {
        channel: Channel::Inbox,
        kind: "user_text".into(),
        payload: serde_json::json!({"text": "review"}),
    }).await.unwrap();

    let turn = turns.create_for_inbox("chat1", &session.id, "msg1", inbox.seq).await.unwrap();
    events.attach_turn(&session.id, inbox.seq, &turn.id).await.unwrap();

    let outbox = events.append_for_chat(&session.id, Some("chat1"), Some(&turn.id), NewEvent {
        channel: Channel::Outbox,
        kind: "agent_text".into(),
        payload: serde_json::json!({"text": "looks good"}),
    }).await.unwrap();

    let trace = turns.trace_for_session(&session.id).await.unwrap();
    assert_eq!(trace[0].turn.id, turn.id);
    assert_eq!(trace[0].events[0].seq, inbox.seq);
    assert_eq!(trace[0].events[1].seq, outbox.seq);
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test store_turns
```

Expected: FAIL because turn storage and event fields do not exist.

**Step 3: Add schema**

Create `migrations/0005_turns_and_metrics.sql`:

```sql
ALTER TABLE events ADD COLUMN chat_id TEXT REFERENCES chats(id);
ALTER TABLE events ADD COLUMN turn_id TEXT;

CREATE TABLE agent_turns (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES sessions(id),
  chat_id TEXT NOT NULL REFERENCES chats(id),
  chat_message_id TEXT NOT NULL REFERENCES chat_messages(id),
  inbox_seq INTEGER NOT NULL,
  status TEXT NOT NULL,
  received_at INTEGER NOT NULL,
  activity_started_at INTEGER,
  ended_at INTEGER
);

CREATE INDEX agent_turns_session ON agent_turns(session_id, received_at DESC);
CREATE INDEX agent_turns_chat ON agent_turns(chat_id, received_at DESC);

CREATE TABLE turn_metrics (
  turn_id TEXT PRIMARY KEY REFERENCES agent_turns(id) ON DELETE CASCADE,
  input_tokens INTEGER,
  output_tokens INTEGER,
  cache_read_tokens INTEGER,
  cache_write_tokens INTEGER,
  tool_call_count INTEGER NOT NULL DEFAULT 0,
  outbox_write_count INTEGER NOT NULL DEFAULT 0,
  error_count INTEGER NOT NULL DEFAULT 0,
  updated_at INTEGER NOT NULL
);
```

**Step 4: Implement EventStore extensions**

Add:

- `append_for_chat(session_id, chat_id, turn_id, NewEvent)`
- `attach_turn(session_id, seq, turn_id)`
- `read_by_chat(chat_id, since, channels)`
- `read_by_turn(turn_id)`

Keep existing `append()` as a wrapper for non-chat session events.

**Step 5: Implement TurnStore**

Implement:

- `create_for_inbox(chat_id, session_id, chat_message_id, inbox_seq)`
- `mark_activity_started(turn_id)`
- `complete(turn_id, status)`
- `record_token_usage(turn_id, usage)`
- `increment_tool_calls(turn_id)`
- `increment_outbox_writes(turn_id)`
- `increment_errors(turn_id)`
- `trace_for_session(session_id)`
- `trace_for_chat(chat_id)`

**Step 6: Run tests**

Run:

```bash
cargo test --test store_turns
cargo test
```

Expected: PASS.

**Step 7: Commit**

Run:

```bash
git add src/model.rs src/store/mod.rs src/store/events.rs src/store/turns.rs migrations/0005_turns_and_metrics.sql tests/store_turns.rs
git commit -m "feat: store agent turns and metrics"
```

## Chat Routing API

### Task 8: Implement Chat Creation, Transcript Reads, And Streams

**Files:**
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/api/mod.rs`
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/api/router.rs`
- Create: `/Users/noelkwan/projects/the-controller-daemon/src/api/chats.rs`
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/state.rs`
- Modify: `/Users/noelkwan/projects/the-controller-daemon/tests/common/mod.rs`
- Create: `/Users/noelkwan/projects/the-controller-daemon/tests/http_chats.rs`

**Step 1: Write failing HTTP test**

Create `tests/http_chats.rs`:

```rust
mod common;

#[tokio::test]
async fn creates_chat_without_starting_agent_session() {
    let (_tmp, base, tok) = common::start().await;
    let c = reqwest::Client::new();

    let r = c.post(format!("{base}/chats"))
        .bearer_auth(&tok)
        .json(&serde_json::json!({
            "project_id": "controller-project",
            "title": "New chat"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(r.status(), 201);
    let chat: serde_json::Value = r.json().await.unwrap();
    assert_eq!(chat["project_id"], "controller-project");

    let sessions: Vec<serde_json::Value> = c.get(format!("{base}/sessions"))
        .bearer_auth(&tok)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(sessions.is_empty());
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test http_chats creates_chat_without_starting_agent_session
```

Expected: FAIL because `/chats` does not exist.

**Step 3: Add ChatStore to AppState**

In `src/state.rs`, add:

```rust
pub chats: ChatStore,
pub turns: TurnStore,
```

Update `src/main.rs` and `tests/common/mod.rs` to construct both stores.

**Step 4: Implement routes**

Create `src/api/chats.rs` with:

```text
GET    /chats
POST   /chats
GET    /chats/:id
DELETE /chats/:id
GET    /chats/:id/transcript
GET    /chats/:id/stream
POST   /chats/:id/workspace-links
PATCH  /chats/:id/workspace-links/:link_id/focus
```

For this task, implement only `GET/POST/GET transcript/stream` and return `501 Not Implemented` for message send, deletion, and workspace mutation until later tasks.

Transcript should return chat user messages plus outbox events only. Do not include raw runtime diagnostics.

**Step 5: Add stream replay**

`GET /chats/:id/stream` should subscribe before replay, like the existing session stream. It must replay by chat cursor and then live-publish durable chat events.

**Step 6: Run tests**

Run:

```bash
cargo test --test http_chats
cargo test
```

Expected: PASS.

**Step 7: Commit**

Run:

```bash
git add src/api/mod.rs src/api/router.rs src/api/chats.rs src/state.rs src/main.rs tests/common/mod.rs tests/http_chats.rs
git commit -m "feat: add chat API shell"
```

### Task 9: Implement Reusable Agent Linking And Fan-Out

**Files:**
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/api/chats.rs`
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/api/agents.rs`
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/api/sessions.rs`
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/supervisor.rs`
- Modify: `/Users/noelkwan/projects/the-controller-daemon/tests/http_chats.rs`

**Step 1: Write failing fan-out test**

Add to `tests/http_chats.rs`:

```rust
#[tokio::test]
async fn sending_message_creates_one_inbox_and_turn_per_linked_reusable_agent() {
    let (_tmp, base, tok, _guard) = common::start_with_env(vec![
        ("TCD_AGENT_CODEX_BINARY", env!("CARGO_BIN_EXE_fake_agent")),
        ("TCD_AGENT_CODEX_SCRIPT", "hold"),
    ]).await;
    let c = reqwest::Client::new();

    common::register_profile(&c, &base, &tok, _tmp.path(), "reviewer").await;
    let chat = common::create_chat(&c, &base, &tok).await;

    let r = c.post(format!("{base}/chats/{}/messages", chat["id"].as_str().unwrap()))
        .bearer_auth(&tok)
        .json(&serde_json::json!({
            "idempotency_id": "client-1",
            "body": "Please review @reviewer",
            "tokens": [
                { "kind": "reusable", "handle": "reviewer", "start": 14, "end": 23 }
            ]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(r.status(), 202);
    let accepted: serde_json::Value = r.json().await.unwrap();
    assert_eq!(accepted["turns"].as_array().unwrap().len(), 1);

    let events: Vec<serde_json::Value> = c.get(format!("{base}/sessions/{}/messages", accepted["turns"][0]["session_id"].as_str().unwrap()))
        .bearer_auth(&tok)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(events.iter().any(|e| e["channel"] == "inbox" && e["kind"] == "user_text"));
}
```

Add helper functions in `tests/common/mod.rs` only after this test fails.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test http_chats sending_message_creates_one_inbox_and_turn_per_linked_reusable_agent
```

Expected: FAIL because message send/fan-out is missing.

**Step 3: Implement token request types**

In `src/api/chats.rs`, define:

```rust
#[derive(Deserialize)]
struct SendChatMessageBody {
    idempotency_id: Option<String>,
    body: String,
    #[serde(default)]
    tokens: Vec<RouteToken>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
enum RouteToken {
    Reusable { handle: String, start: usize, end: usize },
    Shadow { handle: String, start: usize, end: usize },
}
```

**Step 4: Resolve reusable profiles**

For each reusable token:

- resolve active profile by handle;
- create or reuse a reusable session for that profile/version;
- insert or update `chat_agent_links`;
- set focused link for the last reusable token in the message.

**Step 5: Fan out to all linked agents**

For every current link:

- append the chat message once;
- append one inbox event per linked session with `chat_id`;
- create one `agent_turn`;
- attach the inbox event to that turn;
- call `dispatch_pending()` on the matching supervisor if live.

**Step 6: Run tests**

Run:

```bash
cargo test --test http_chats sending_message_creates_one_inbox_and_turn_per_linked_reusable_agent
cargo test --test http_chats
cargo test
```

Expected: PASS.

**Step 7: Commit**

Run:

```bash
git add src/api/chats.rs src/api/agents.rs src/api/sessions.rs src/supervisor.rs tests/http_chats.rs tests/common/mod.rs
git commit -m "feat: fan out chat messages to reusable agents"
```

### Task 10: Implement Shadow Sessions And Chat Deletion Cleanup

**Files:**
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/api/chats.rs`
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/api/sessions.rs`
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/store/chats.rs`
- Modify: `/Users/noelkwan/projects/the-controller-daemon/tests/http_chats.rs`

**Step 1: Write failing shadow spawn test**

Add:

```rust
#[tokio::test]
async fn shadow_agent_spawns_on_send_and_is_stopped_on_chat_delete() {
    let (_tmp, base, tok, _guard) = common::start_with_env(vec![
        ("TCD_AGENT_CODEX_BINARY", env!("CARGO_BIN_EXE_fake_agent")),
        ("TCD_AGENT_CODEX_SCRIPT", "hold"),
    ]).await;
    let c = reqwest::Client::new();
    common::register_profile(&c, &base, &tok, _tmp.path(), "debugger").await;
    let chat = common::create_chat(&c, &base, &tok).await;
    let chat_id = chat["id"].as_str().unwrap();

    let r = c.post(format!("{base}/chats/{chat_id}/messages"))
        .bearer_auth(&tok)
        .json(&serde_json::json!({
            "body": "Investigate %debugger",
            "tokens": [{ "kind": "shadow", "handle": "debugger", "start": 12, "end": 21 }]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 202);
    let accepted: serde_json::Value = r.json().await.unwrap();
    let session_id = accepted["turns"][0]["session_id"].as_str().unwrap();

    c.delete(format!("{base}/chats/{chat_id}"))
        .bearer_auth(&tok)
        .send()
        .await
        .unwrap();

    let session: serde_json::Value = c.get(format!("{base}/sessions/{session_id}"))
        .bearer_auth(&tok)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(session["status"], "ended");
    assert_eq!(session["end_reason"], "owner_chat_deleted");
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test http_chats shadow_agent_spawns_on_send_and_is_stopped_on_chat_delete
```

Expected: FAIL because shadow ownership and cleanup are missing.

**Step 3: Implement shadow token handling**

For each `%agent` token:

- resolve active profile by handle;
- create a new session with `session_kind = Shadow`;
- set `owner_chat_id = chat.id`;
- link it to the chat with `route_type = shadow`;
- do not reuse existing shadow sessions.

**Step 4: Implement delete semantics**

`DELETE /chats/:id` must:

- load `chat_agent_links`;
- for shadow links, call supervisor `stop()` and set session end reason to `owner_chat_deleted`;
- for reusable links, leave sessions running;
- mark chat deleted;
- prevent deleted chats from accepting new messages.

**Step 5: Run tests**

Run:

```bash
cargo test --test http_chats shadow_agent_spawns_on_send_and_is_stopped_on_chat_delete
cargo test --test http_chats
cargo test
```

Expected: PASS.

**Step 6: Commit**

Run:

```bash
git add src/api/chats.rs src/api/sessions.rs src/store/chats.rs tests/http_chats.rs
git commit -m "feat: manage shadow chat sessions"
```

### Task 11: Add Workspace Link API And Context Snapshots

**Files:**
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/api/chats.rs`
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/store/chats.rs`
- Modify: `/Users/noelkwan/projects/the-controller-daemon/src/profile_fs.rs`
- Modify: `/Users/noelkwan/projects/the-controller-daemon/tests/http_chats.rs`

**Step 1: Write failing workspace link test**

Add:

```rust
#[tokio::test]
async fn workspace_link_stores_controller_snapshot_and_focus() {
    let (_tmp, base, tok) = common::start().await;
    let c = reqwest::Client::new();
    let chat = common::create_chat(&c, &base, &tok).await;
    let chat_id = chat["id"].as_str().unwrap();

    let r = c.post(format!("{base}/chats/{chat_id}/workspace-links"))
        .bearer_auth(&tok)
        .json(&serde_json::json!({
            "project_id": "controller-project",
            "workspace_id": "workspace-1",
            "path": "/tmp/controller-worktree",
            "label": "controller-worktree",
            "branch": "codex/chat-routing",
            "focused": true
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(r.status(), 201);
    let link: serde_json::Value = r.json().await.unwrap();
    assert_eq!(link["path"], "/tmp/controller-worktree");
    assert_eq!(link["focused"], true);
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test http_chats workspace_link_stores_controller_snapshot_and_focus
```

Expected: FAIL because workspace link routes are missing.

**Step 3: Implement link routes**

Implement:

```text
POST  /chats/:id/workspace-links
PATCH /chats/:id/workspace-links/:link_id/focus
```

Validate:

- `path` must be absolute.
- daemon stores the path but does not create or delete it.
- focus change clears previous focused workspace links for the chat.

**Step 4: Add context rendering helper**

In `profile_fs.rs`, add a pure function:

```rust
pub fn render_workspace_context(links: &[ChatWorkspaceLink]) -> String
```

It returns deterministic Markdown for AGENTS.md updates. The function should not write files in this task.

**Step 5: Run tests**

Run:

```bash
cargo test --test http_chats workspace_link_stores_controller_snapshot_and_focus
cargo test
```

Expected: PASS.

**Step 6: Commit**

Run:

```bash
git add src/api/chats.rs src/store/chats.rs src/profile_fs.rs tests/http_chats.rs
git commit -m "feat: store chat workspace snapshots"
```

## Controller Backend Integration

### Task 12: Proxy HTTP Gateway Requests Over UDS

**Files:**
- Modify: `server/Cargo.toml`
- Modify: `server/src/daemon_gateway.rs`
- Modify: `server/src/main.rs`
- Modify: `server/tests/daemon_gateway.rs`

**Step 1: Write failing proxy test**

In `server/tests/daemon_gateway.rs`, add a test that starts a local Unix socket test server and verifies `GET /health` is forwarded without exposing auth to the caller. If a full axum gateway integration test is too heavy, test the gateway client function directly:

```rust
#[tokio::test]
async fn gateway_health_uses_unix_socket() {
    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("daemon.sock");
    let server = the_controller_lib::daemon_gateway::test_support::serve_uds_health(&socket).await;

    let body = the_controller_lib::daemon_gateway::forward_http_for_test(&socket, "/health").await.unwrap();

    assert_eq!(body.status, 200);
    assert_eq!(body.body, "ok");
    server.abort();
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cd server && cargo test --test daemon_gateway gateway_health_uses_unix_socket
```

Expected: FAIL because UDS forwarding does not exist.

**Step 3: Implement UDS client**

Use one of these implementation options:

- Preferred if available: `reqwest` with its Unix socket feature.
- Fallback: `hyper-util` with `tokio::net::UnixStream`.

Expose:

```rust
pub async fn forward_http(
    socket_path: &std::path::Path,
    method: axum::http::Method,
    daemon_path: String,
    body: bytes::Bytes,
) -> Result<DaemonResponse, String>
```

`DaemonResponse` should carry status, content type, and body bytes.

**Step 4: Wire axum gateway route**

In `server/src/main.rs`:

- Route `/api/daemon/{*path}` for GET, POST, PATCH, DELETE.
- Normalize to daemon path.
- Forward request body.
- Return status and content type from daemon.
- Map daemon connection failure to `502 Bad Gateway`.

Do not read or return `daemon.token`.

**Step 5: Run tests**

Run:

```bash
cd server && cargo test --test daemon_gateway
cd server && cargo test
```

Expected: PASS.

**Step 6: Commit**

Run:

```bash
git add server/Cargo.toml server/src/daemon_gateway.rs server/src/main.rs server/tests/daemon_gateway.rs
git commit -m "feat: proxy daemon http over unix socket"
```

### Task 13: Proxy Daemon WebSockets Over UDS

**Files:**
- Modify: `server/src/daemon_gateway.rs`
- Modify: `server/src/main.rs`
- Create: `server/tests/daemon_gateway_ws.rs`
- Modify: `src/lib/daemon/stream.test.ts`

**Step 1: Write failing frontend stream test**

In `src/lib/daemon/stream.test.ts`, assert stream URLs are same-origin:

```ts
it("opens the controller gateway stream route", () => {
  const ws = vi.fn();
  vi.stubGlobal("WebSocket", ws as any);
  daemonStore.client = new DaemonClient("/api/daemon");

  const handle = openStream("s1");

  expect(ws).toHaveBeenCalledWith("/api/daemon/sessions/s1/stream?since=0");
  handle.close();
});
```

**Step 2: Run test to verify it fails**

Run:

```bash
pnpm test src/lib/daemon/stream.test.ts
```

Expected: FAIL until the client URL and gateway route are aligned.

**Step 3: Write backend WS proxy test**

Create `server/tests/daemon_gateway_ws.rs` with a focused test around gateway URL classification if full WS over UDS is too large for unit scope. The test must fail until the route recognizes stream paths:

```rust
#[test]
fn stream_paths_are_websocket_gateway_paths() {
    assert!(the_controller_lib::daemon_gateway::is_daemon_stream_path("/api/daemon/chats/c1/stream"));
    assert!(the_controller_lib::daemon_gateway::is_daemon_stream_path("/api/daemon/sessions/s1/stream"));
    assert!(!the_controller_lib::daemon_gateway::is_daemon_stream_path("/api/daemon/chats"));
}
```

**Step 4: Implement WS proxy**

In `server/src/main.rs`, add GET routes for:

```text
/api/daemon/chats/:id/stream
/api/daemon/sessions/:id/stream
```

The handler upgrades browser WS, opens a daemon WS over UDS, and relays frames both directions.

If the first implementation cannot support WS over UDS with current dependencies, implement the explicit fallback route as server-sent events only after changing frontend `openStream` and tests to match. Do not use daemon TCP fallback.

**Step 5: Run tests**

Run:

```bash
pnpm test src/lib/daemon/stream.test.ts
cd server && cargo test --test daemon_gateway_ws
cd server && cargo test
```

Expected: PASS.

**Step 6: Commit**

Run:

```bash
git add src/lib/daemon/stream.test.ts server/src/daemon_gateway.rs server/src/main.rs server/tests/daemon_gateway_ws.rs
git commit -m "feat: proxy daemon streams through controller"
```

### Task 14: Add Controller Workspace Snapshot Commands

**Files:**
- Modify: `server/src/models.rs`
- Modify: `server/src/storage.rs`
- Modify: `server/src/commands.rs`
- Modify: `server/src/main.rs`
- Create: `server/tests/chat_workspace_links.rs`

**Step 1: Write failing backend test**

Create `server/tests/chat_workspace_links.rs`:

```rust
use the_controller_lib::models::ChatWorkspaceSnapshot;

#[test]
fn workspace_snapshot_contains_controller_owned_fields_only() {
    let snapshot = ChatWorkspaceSnapshot {
        project_id: "p1".into(),
        workspace_id: "w1".into(),
        path: "/tmp/worktree".into(),
        label: "feature-worktree".into(),
        branch: Some("codex/feature".into()),
        focused: true,
    };

    assert_eq!(snapshot.path, "/tmp/worktree");
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cd server && cargo test --test chat_workspace_links
```

Expected: FAIL because the snapshot type does not exist.

**Step 3: Implement snapshot model**

In `server/src/models.rs`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatWorkspaceSnapshot {
    pub project_id: String,
    pub workspace_id: String,
    pub path: String,
    pub label: String,
    pub branch: Option<String>,
    pub focused: bool,
}
```

**Step 4: Implement command helper**

Add a pure helper in `commands.rs` that converts a `Project` and `SessionConfig` into `ChatWorkspaceSnapshot`. It must not call the daemon. The HTTP handler will post the resulting JSON to `/api/daemon/chats/:id/workspace-links`.

**Step 5: Add route for workspace chat creation**

Add a Controller route that:

- creates or receives a chat id from the daemon gateway;
- creates any Controller-owned worktree using existing worktree conventions;
- posts the snapshot to the daemon;
- offloads filesystem/git work with `spawn_blocking`.

**Step 6: Run tests**

Run:

```bash
cd server && cargo test --test chat_workspace_links
cd server && cargo test
```

Expected: PASS.

**Step 7: Commit**

Run:

```bash
git add server/src/models.rs server/src/storage.rs server/src/commands.rs server/src/main.rs server/tests/chat_workspace_links.rs
git commit -m "feat: build controller workspace snapshots"
```

## Frontend Product Modes

### Task 15: Expand Daemon Types And Store Around Profiles, Chats, Links, And Turns

**Files:**
- Modify: `src/lib/daemon/types.ts`
- Modify: `src/lib/daemon/client.ts`
- Modify: `src/lib/daemon/store.svelte.ts`
- Modify: `src/lib/daemon/types.test.ts`
- Modify: `src/lib/daemon/client.test.ts`
- Modify: `src/lib/daemon/store.test.ts`

**Step 1: Write failing type/client tests**

In `src/lib/daemon/client.test.ts`, add:

```ts
it("creates a chat through the gateway", async () => {
  const fetchMock = mockFetch([{ status: 201, body: { id: "c1", project_id: "p1", title: "New chat" } }]);
  vi.stubGlobal("fetch", fetchMock);

  const c = new DaemonClient("/api/daemon");
  const chat = await c.createChat({ project_id: "p1", title: "New chat" });

  expect(chat.id).toBe("c1");
  expect(fetchMock.mock.calls[0][0]).toBe("/api/daemon/chats");
});
```

Add similar tests for `listProfiles`, `sendChatMessage`, and `getAgentTrace`.

**Step 2: Run test to verify it fails**

Run:

```bash
pnpm test src/lib/daemon/client.test.ts
```

Expected: FAIL because methods/types do not exist.

**Step 3: Add types**

In `src/lib/daemon/types.ts`, add:

- `AgentProfile`
- `AgentProfileVersion`
- `Chat`
- `ChatMessage`
- `ChatAgentLink`
- `ChatWorkspaceLink`
- `RouteToken`
- `SendChatMessageRequest`
- `AgentTurn`
- `AgentTurnTrace`
- `ChatMetrics`

Keep old session event types until replaced by chat routes.

**Step 4: Add client methods**

Add methods:

- `listProfiles`
- `saveProfile`
- `archiveProfile`
- `restoreProfile`
- `testProfileInChat`
- `listChats`
- `createChat`
- `deleteChat`
- `sendChatMessage`
- `readChatTranscript`
- `chatStreamUrl`
- `addWorkspaceLink`
- `focusWorkspaceLink`
- `getAgentTrace`
- `getChatMetrics`

**Step 5: Update store state**

Store should have separate maps for:

- profiles;
- chats;
- active chat id;
- chat transcripts;
- chat summaries;
- agent traces;

Do not remove session map until all existing components are migrated.

**Step 6: Run tests**

Run:

```bash
pnpm test src/lib/daemon/types.test.ts src/lib/daemon/client.test.ts src/lib/daemon/store.test.ts
pnpm test
```

Expected: PASS.

**Step 7: Commit**

Run:

```bash
git add src/lib/daemon/types.ts src/lib/daemon/client.ts src/lib/daemon/store.svelte.ts src/lib/daemon/*.test.ts
git commit -m "feat: model daemon chats and profiles in frontend"
```

### Task 16: Add Agent Creation Workspace Mode

**Files:**
- Modify: `src/lib/stores.ts`
- Modify: `src/lib/WorkspaceModePicker.svelte`
- Modify: `src/App.svelte`
- Create: `src/lib/agents/AgentCreationWorkspace.svelte`
- Create: `src/lib/agents/ProfileList.svelte`
- Create: `src/lib/agents/ProfileEditor.svelte`
- Create: `src/lib/agents/ProfilePreviewDrawer.svelte`
- Create: `src/lib/agents/profile-validation.ts`
- Create: `src/lib/agents/AgentCreationWorkspace.test.svelte.ts`
- Create: `src/lib/agents/profile-validation.test.ts`

**Step 1: Write failing validation test**

Create `src/lib/agents/profile-validation.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { validateProfileDraft } from "./profile-validation";

describe("validateProfileDraft", () => {
  it("blocks invalid handles and empty prompts", () => {
    const result = validateProfileDraft({
      name: "Reviewer",
      handle: "Reviewer!",
      runtime: "codex",
      prompt: "",
      skills: [],
      outbox_instructions: "",
      default_workspace_behavior: "focused",
    });

    expect(result.blocking.map((x) => x.field)).toEqual(expect.arrayContaining(["handle", "prompt"]));
  });
});
```

**Step 2: Run test to verify it fails**

Run:

```bash
pnpm test src/lib/agents/profile-validation.test.ts
```

Expected: FAIL because module does not exist.

**Step 3: Implement validation helper**

Create a pure validation helper matching PRD rules:

- name required;
- handle required;
- handle uses lowercase letters, numbers, hyphens;
- runtime required;
- prompt required;
- missing skills/outbox/workspace behavior produce warnings, not blockers.

**Step 4: Add workspace mode**

In `stores.ts`, extend:

```ts
export type WorkspaceMode = "agents" | "kanban" | "chat" | "agent-create" | "agent-observe";
```

Update `WorkspaceModePicker.svelte` with compact entries for `Create` and `Observe`. Keep existing `Agents` mode until old maintainer/auto-worker dashboard is renamed.

**Step 5: Build Agent Creation UI**

Create:

- profile list with search and active/archived filter;
- editor fields from PRD;
- preview drawer;
- explicit save;
- duplicate/archive/restore/test buttons;
- initials avatar fallback;
- dirty indicator.

Use existing Catppuccin variables in `src/app.css`. Do not add nested cards.

**Step 6: Write component test**

In `AgentCreationWorkspace.test.svelte.ts`, verify:

- empty state shows `No agent profiles`;
- `New Profile` opens editor draft;
- invalid handle disables save;
- valid save calls `daemonStore.client.saveProfile`.

**Step 7: Run tests**

Run:

```bash
pnpm test src/lib/agents/profile-validation.test.ts src/lib/agents/AgentCreationWorkspace.test.svelte.ts
pnpm test
```

Expected: PASS.

**Step 8: Commit**

Run:

```bash
git add src/lib/stores.ts src/lib/WorkspaceModePicker.svelte src/App.svelte src/lib/agents
git commit -m "feat: add agent creation workspace"
```

### Task 17: Implement Composer-First Chat Creation

**Files:**
- Modify: `src/lib/chat/ChatWorkspace.svelte`
- Modify: `src/lib/chat/ChatView.svelte`
- Modify: `src/lib/chat/ChatInput.svelte`
- Modify: `src/lib/chat/ChatSessionList.svelte`
- Modify: `src/lib/HotkeyManager.svelte`
- Modify: `src/lib/Sidebar.svelte`
- Modify: `src/lib/chat/ChatWorkspace.test.ts`
- Modify: `src/lib/chat/ChatInput.test.ts`
- Create: `src/lib/chat/chat-routing.ts`
- Create: `src/lib/chat/chat-routing.test.ts`

**Step 1: Write failing routing helper test**

Create `src/lib/chat/chat-routing.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { extractRouteTokenQuery } from "./chat-routing";

describe("extractRouteTokenQuery", () => {
  it("detects @ and % token queries at the cursor", () => {
    expect(extractRouteTokenQuery("ask @rev", 8)).toEqual({ kind: "reusable", query: "rev", start: 4, end: 8 });
    expect(extractRouteTokenQuery("ask %debug", 10)).toEqual({ kind: "shadow", query: "debug", start: 4, end: 10 });
  });
});
```

**Step 2: Run test to verify it fails**

Run:

```bash
pnpm test src/lib/chat/chat-routing.test.ts
```

Expected: FAIL because helper does not exist.

**Step 3: Implement token helper**

Implement pure helpers for:

- detecting current token query;
- inserting durable route token metadata;
- mapping selected profile to `RouteToken`.

Do not rely only on post-submit string parsing.

**Step 4: Write failing new-chat test**

Update `ChatWorkspace.test.ts` or create a Svelte component test to verify `n` creates a chat without opening `NewChatDialog`.

Expected behavior:

- `daemonStore.client.createChat` called with focused project id.
- `daemonStore.activeChatId` set.
- composer receives focus.
- no agent selection modal appears.

**Step 5: Implement composer-first flow**

- Replace default modal path for `n` in chat mode.
- Keep advanced `NewChatDialog` only if reachable from a secondary action.
- Update `Esc` in composer to focus active chat row.
- Keep `Shift+Esc` interrupt behavior.

**Step 6: Run tests**

Run:

```bash
pnpm test src/lib/chat/chat-routing.test.ts src/lib/chat/ChatWorkspace.test.ts src/lib/chat/ChatInput.test.ts
pnpm test
```

Expected: PASS.

**Step 7: Commit**

Run:

```bash
git add src/lib/chat src/lib/HotkeyManager.svelte src/lib/Sidebar.svelte
git commit -m "feat: create chats from composer flow"
```

### Task 18: Add Agent Token Suggestions And Summary Pane

**Files:**
- Modify: `src/lib/chat/ChatInput.svelte`
- Modify: `src/lib/chat/ChatView.svelte`
- Create: `src/lib/chat/AgentTokenMenu.svelte`
- Create: `src/lib/chat/ChatSummaryPane.svelte`
- Modify: `src/lib/chat/Transcript.svelte`
- Create: `src/lib/chat/AgentTokenMenu.test.svelte.ts`
- Create: `src/lib/chat/ChatSummaryPane.test.svelte.ts`
- Modify: `src/lib/chat/ChatInput.test.ts`

**Step 1: Write failing token menu test**

Create `AgentTokenMenu.test.svelte.ts`:

```ts
import { render, fireEvent } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import AgentTokenMenu from "./AgentTokenMenu.svelte";

describe("AgentTokenMenu", () => {
  it("labels reusable and shadow selections differently", async () => {
    const onSelect = vi.fn();
    const { getByText } = render(AgentTokenMenu, {
      kind: "shadow",
      query: "rev",
      profiles: [{ id: "p1", handle: "reviewer", name: "Reviewer", runtime: "codex", archived_at: null }],
      onSelect,
    });

    await fireEvent.click(getByText("%reviewer"));
    expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({ kind: "shadow", handle: "reviewer" }));
  });
});
```

**Step 2: Run test to verify it fails**

Run:

```bash
pnpm test src/lib/chat/AgentTokenMenu.test.svelte.ts
```

Expected: FAIL because component does not exist.

**Step 3: Implement menu**

`AgentTokenMenu` should:

- filter active profiles by handle/name;
- render `@handle` for reusable and `%handle` for shadow;
- support keyboard navigation;
- call `onSelect` with stable profile id and handle.

**Step 4: Implement summary pane**

`ChatSummaryPane` should show:

- all linked agents;
- focused agent;
- all linked workspaces;
- focused workspace;
- quiet `none` state when empty.

**Step 5: Integrate send path**

`ChatInput` sends:

```ts
await daemonStore.client.sendChatMessage(activeChatId, {
  body: value,
  tokens: routeTokens,
  idempotency_id: crypto.randomUUID(),
});
```

If no agent is associated and no token is present, show a local validation error instead of silently sending.

**Step 6: Run tests**

Run:

```bash
pnpm test src/lib/chat/AgentTokenMenu.test.svelte.ts src/lib/chat/ChatSummaryPane.test.svelte.ts src/lib/chat/ChatInput.test.ts
pnpm test
```

Expected: PASS.

**Step 7: Commit**

Run:

```bash
git add src/lib/chat
git commit -m "feat: add chat agent routing controls"
```

### Task 19: Add Chat Metrics Tab And Agent Observability Workspace

**Files:**
- Modify: `src/lib/stores.ts`
- Modify: `src/App.svelte`
- Modify: `src/lib/chat/ChatView.svelte`
- Create: `src/lib/chat/ChatMetricsTab.svelte`
- Create: `src/lib/observability/AgentObservabilityWorkspace.svelte`
- Create: `src/lib/observability/AgentTraceView.svelte`
- Create: `src/lib/observability/turn-format.ts`
- Create: `src/lib/observability/turn-format.test.ts`
- Create: `src/lib/observability/AgentTraceView.test.svelte.ts`

**Step 1: Write failing formatter test**

Create `turn-format.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { formatMetricValue } from "./turn-format";

describe("formatMetricValue", () => {
  it("distinguishes missing metrics from zero", () => {
    expect(formatMetricValue(null, "tokens")).toBe("unavailable");
    expect(formatMetricValue(0, "tokens")).toBe("0 tokens");
  });
});
```

**Step 2: Run test to verify it fails**

Run:

```bash
pnpm test src/lib/observability/turn-format.test.ts
```

Expected: FAIL because module does not exist.

**Step 3: Implement metrics tab**

In `ChatView.svelte`:

- add `Chat` and `Metrics` tabs;
- default to `Chat`;
- do not add a `Thinking` tab;
- load chat metrics from daemon client;
- link agent rows to `agent-observe` mode.

**Step 4: Implement agent trace workspace**

Create a turn-first page:

- agent status/ownership header;
- linked chats/workspaces;
- current or most recent turn;
- reverse-chronological turn list;
- expandable details for thinking, tools, outbox, tokens, timing, errors.

**Step 5: Run tests**

Run:

```bash
pnpm test src/lib/observability/turn-format.test.ts src/lib/observability/AgentTraceView.test.svelte.ts
pnpm test
```

Expected: PASS.

**Step 6: Commit**

Run:

```bash
git add src/lib/stores.ts src/App.svelte src/lib/chat/ChatView.svelte src/lib/chat/ChatMetricsTab.svelte src/lib/observability
git commit -m "feat: add agent observability views"
```

## Avatar Jobs And Polish

### Task 20: Add Background Avatar Job Scheduling In Controller

**Files:**
- Modify: `server/src/models.rs`
- Create: `server/src/avatar_jobs.rs`
- Modify: `server/src/lib.rs`
- Modify: `server/src/main.rs`
- Create: `server/tests/avatar_jobs.rs`
- Modify: `src/lib/agents/ProfileEditor.svelte`

**Step 1: Write failing avatar prompt test**

Create `server/tests/avatar_jobs.rs`:

```rust
use the_controller_lib::avatar_jobs::build_avatar_prompt;

#[test]
fn avatar_prompt_requires_pixel_art_256_humanoid() {
    let prompt = build_avatar_prompt("Reviewer", "Reviews code carefully");

    assert!(prompt.contains("256 by 256"));
    assert!(prompt.contains("pixel-art"));
    assert!(prompt.contains("humanoid"));
    assert!(prompt.contains("Reviewer"));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cd server && cargo test --test avatar_jobs
```

Expected: FAIL because `avatar_jobs` does not exist.

**Step 3: Implement job module**

Add:

- `build_avatar_prompt(name, description)`;
- `schedule_avatar_generation(profile_id, profile_name, description)`;
- a background task that runs Codex CLI with the image generation request;
- result storage path under Controller state, such as `~/.the-controller/assets/agent-avatars/<profile_id>.png`;
- success/failure callback to daemon `/profiles/:id/avatar` or direct gateway route if added.

Do not block profile save or `Test in Chat`.

**Step 4: Add retry UI**

In `ProfileEditor.svelte`, show initials avatar while pending/failed and expose retry for saved profiles.

**Step 5: Run tests**

Run:

```bash
cd server && cargo test --test avatar_jobs
cd server && cargo test
pnpm test src/lib/agents/AgentCreationWorkspace.test.svelte.ts
```

Expected: PASS.

**Step 6: Commit**

Run:

```bash
git add server/src/models.rs server/src/avatar_jobs.rs server/src/lib.rs server/src/main.rs server/tests/avatar_jobs.rs src/lib/agents/ProfileEditor.svelte
git commit -m "feat: schedule agent avatar generation"
```

## End-To-End Validation

### Task 21: Add End-To-End Agent Routing Evaluation

**Files:**
- Create: `e2e/specs/agent-routing.spec.ts`
- Modify: `playwright.config.ts` only if needed.
- Modify: `e2e/eval.sh` only if needed.

**Step 1: Write failing Playwright test**

Create `e2e/specs/agent-routing.spec.ts`:

```ts
import { test, expect } from "@playwright/test";

test("creates profile, routes chat message, and shows outbox reply only", async ({ page }) => {
  await page.goto("/");
  await page.keyboard.press("Meta+k");
  await page.keyboard.press("c");

  await page.getByRole("button", { name: "New Profile" }).click();
  await page.getByLabel("Name").fill("Reviewer");
  await page.getByLabel("Handle").fill("reviewer");
  await page.getByLabel("Prompt").fill("Review carefully.");
  await page.getByRole("button", { name: "Save" }).click();

  await page.keyboard.press("Meta+k");
  await page.keyboard.press("c");
  await page.keyboard.press("n");
  await page.getByRole("textbox", { name: "Chat input" }).fill("Please review @reviewer");
  await page.keyboard.press("Meta+Enter");

  await expect(page.getByText("@reviewer")).toBeVisible();
  await expect(page.getByText("raw stdout")).toHaveCount(0);
});
```

Adjust selectors to actual labels after UI implementation.

**Step 2: Run test to verify it fails or is skipped for missing daemon harness**

Run:

```bash
pnpm exec playwright test e2e/specs/agent-routing.spec.ts
```

Expected before harness work: FAIL because daemon/test setup is incomplete.

**Step 3: Add daemon harness**

Update `e2e/eval.sh` or Playwright setup to:

- start `the-controller-daemon` with `TCD_STATE_DIR` pointing at a temp dir;
- set `TCD_AGENT_CODEX_BINARY` to the daemon fake agent;
- start Controller via `./dev.sh` or equivalent;
- clean up processes and temp dirs.

Do not start daemon TCP.

**Step 4: Run E2E**

Run:

```bash
pnpm exec playwright test e2e/specs/agent-routing.spec.ts
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add e2e/specs/agent-routing.spec.ts e2e/eval.sh playwright.config.ts
git commit -m "test: cover agent routing end to end"
```

### Task 22: Final Architecture Verification Checklist

**Files:**
- Modify: tests only if gaps are found.

**Step 1: Run daemon full test suite**

Run from daemon repo:

```bash
cargo test
```

Expected: PASS.

**Step 2: Run Controller backend tests**

Run from Controller repo:

```bash
cd server && cargo test
```

Expected: PASS.

**Step 3: Run frontend tests**

Run from Controller repo:

```bash
pnpm test
```

Expected: PASS.

**Step 4: Run E2E**

Run from Controller repo:

```bash
pnpm exec playwright test e2e/specs/agent-routing.spec.ts
```

Expected: PASS.

**Step 5: Verify architecture requirements**

Manually check each item against tests or code:

- Controller restart does not stop daemon sessions.
- Daemon restart preserves chats, links, sessions, events, turns, and pending inbox items.
- Browser code calls only same-origin Controller routes.
- Controller backend talks to the daemon over a Unix socket.
- `@agent` links reusable sessions that survive chat deletion.
- `%agent` creates shadow sessions on send and stops them on chat deletion.
- One chat send fans out to one inbox item and one turn per linked agent.
- Chat transcript rendering excludes raw runtime output.
- Observability can replay by chat, agent session, and turn.

If any item lacks a test, add one before claiming completion.

**Step 6: Commit final fixes**

Run:

```bash
git status --short
git add <changed files>
git commit -m "test: verify agent architecture"
```

Only commit if there are changes.

## Execution Checkpoints

Report for review after each batch:

1. Tasks 1-4: transport and browser-safe gateway.
2. Tasks 5-7: daemon durable model.
3. Tasks 8-11: chat routing API.
4. Tasks 12-14: Controller gateway and workspace snapshots.
5. Tasks 15-19: frontend product modes.
6. Tasks 20-22: avatar jobs and end-to-end validation.

Each report must include:

- commits created in each repo;
- tests run with exact commands and pass/fail status;
- any architecture requirement still untested;
- any changes needed to this plan before the next batch.

