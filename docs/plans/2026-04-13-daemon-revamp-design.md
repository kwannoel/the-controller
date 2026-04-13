# Daemon Revamp — Design

## Problem

The Controller currently interacts with Claude Code through a PTY + tmux layer, with xterm.js rendering raw terminal output. This works, but three limitations keep compounding:

1. **No structured data.** Claude's output is ANSI-escaped terminal bytes. The UI can't reason about thinking blocks, tool calls, or message boundaries without parsing terminal state.
2. **Fragile process control.** Delivering a message means typing into a PTY. There's no inbox, no mid-work notification, no first-class "agent is busy / idle / resumable" state beyond the hook-based socket signals.
3. **tmux-coupled lifecycle.** Session persistence is tied to tmux. Reattachment produces redraw artifacts. The daemon side of the world (spawn, resume, restart) is spread across `pty_manager`, `tmux`, and `status_socket`.

Reference: the slock daemon (`~/projects/slock-daemon-annotated`) spawns Claude with `--output-format stream-json --input-format stream-json`, parses structured events, queues messages in an inbox, and manages agents as a daemon separate from its UI. We want to adopt that pattern.

## Goals

- Structured JSON I/O with Claude (and Codex) — parse `assistant`/`tool_use`/`thinking`/`result` events, not ANSI bytes.
- Agent-style process management — explicit state machine (active/idle/busy), message inbox, session resume.
- Daemon process independent of the Tauri UI — sessions survive UI restarts without tmux.
- Incremental rollout — don't rip out the terminal path until the daemon path is proven.

## Non-goals

- Cloud sync or remote agents. This is a local-only redesign.
- Replacing Claude Code's auth. The daemon spawns `claude` the same way a user would; auth stays with the CLI.
- Changing the worktree / project / session data model. Only the communication layer changes.

## Decisions

| # | Decision | Choice | Rationale |
|---|----------|--------|-----------|
| 1 | Motivation | Structured data + process control + daemon model (all three) | These are complementary; doing only one leaves the others as ongoing sources of complexity. |
| 2 | Daemon topology | **Separate process** from the Tauri app | Sessions outlive the UI without relying on tmux. Clean boundary between UI and process management. |
| 3 | Language | **Rust** | Share models/types with the Tauri backend via a Cargo workspace. No new runtime dependency. |
| 4 | IPC transport | **Unix domain socket, JSON-lines protocol** | Reuses the pattern already established by `status_socket.rs`. No WebSocket/gRPC overhead for a local-only app. |
| 5 | UI strategy | **Incremental replace behind a flag** | Terminal path is load-bearing today. Gate the daemon path per-session or per-project, validate, then retire tmux/xterm. |

## Architecture

```
┌─────────────────────────────────────────────┐
│            Tauri App (UI)                    │
│  Svelte 5 frontend                           │
│  Rust backend (commands.rs, state.rs, …)     │
│                                              │
│   ┌────────────────────────────┐             │
│   │   DaemonClient (new)       │             │
│   │   - Unix socket connection │             │
│   │   - Request/response       │             │
│   │   - Event stream relay     │             │
│   └─────────────┬──────────────┘             │
└─────────────────┼────────────────────────────┘
                  │
           Unix domain socket
           /tmp/the-controller-daemon.sock
           JSON-lines protocol
                  │
┌─────────────────▼────────────────────────────┐
│  controller-daemon (new Rust binary)          │
│                                                │
│   ┌──────────────────────────────────────┐   │
│   │  DaemonServer                         │   │
│   │  - Accept UI connections              │   │
│   │  - Route commands to AgentManager     │   │
│   │  - Broadcast events to subscribers    │   │
│   └──────────────────────────────────────┘   │
│                                                │
│   ┌──────────────────────────────────────┐   │
│   │  AgentManager                         │   │
│   │  - HashMap<SessionId, Agent>          │   │
│   │  - Spawn / stop / deliver / resume    │   │
│   └──────────────────────────────────────┘   │
│                                                │
│   ┌──────────────────────────────────────┐   │
│   │  Agent (per session)                  │   │
│   │  - Driver (Claude | Codex)            │   │
│   │  - State: Active | Idle               │   │
│   │  - Inbox: VecDeque<Message>           │   │
│   │  - stdout parser (stream-json)        │   │
│   │  - stdin writer                       │   │
│   └──────────────────────────────────────┘   │
│                                                │
│   ┌──────────────────────────────────────┐   │
│   │  Drivers                              │   │
│   │  - ClaudeDriver (long-lived, stdin)   │   │
│   │  - CodexDriver (one-shot, resume)     │   │
│   └──────────────────────────────────────┘   │
└────────────────────────────────────────────────┘
      │ stdin/stdout pipes
      ▼
   claude / codex child processes
```

### Cargo workspace

```
the-controller/
├── Cargo.toml                 # workspace root
├── crates/
│   ├── controller-core/       # shared models (SessionConfig, SessionId, events)
│   ├── controller-daemon/     # new: standalone daemon binary
│   └── src-tauri/             # existing Tauri app, now a workspace member
```

`controller-core` owns the types that cross the socket boundary: `DaemonRequest`, `DaemonResponse`, `AgentEvent`, `SessionKind`, `AgentState`. Both the daemon and the Tauri app depend on it.

## Data flow

### Spawn a session

1. User creates a session in UI → Tauri `create_session` command writes `SessionConfig` to `project.json` (unchanged).
2. UI invokes `connect_session` Tauri command (unchanged name, new implementation).
3. Tauri backend sends `DaemonRequest::Spawn { session_id, workdir, kind, initial_prompt, continue_session }` over the daemon socket.
4. Daemon's `AgentManager::spawn` constructs the right driver, spawns `claude --output-format stream-json --input-format stream-json …` (or `codex exec --json …`), and starts:
   - A stdout reader task that parses stream-json into `AgentEvent`s.
   - A stdin writer that holds an mpsc channel for message delivery.
5. Daemon broadcasts `AgentEvent::Started { session_id, pid }` to subscribed UI clients.

### Deliver a message

1. UI sends `DaemonRequest::Deliver { session_id, content }`.
2. Daemon's `Agent::deliver`:
   - If idle: format message as user JSON, write to stdin.
   - If busy: push to inbox, optionally write a notification message.
3. On stream-json `result` event (turn end), agent drains next inbox message or transitions to idle.

### Event relay to UI

Daemon emits `AgentEvent` over each connected UI subscriber's socket:

- `TextDelta { session_id, text }` — assistant text as it arrives.
- `ThinkingDelta { session_id, text }` — extended thinking blocks.
- `ToolCall { session_id, name, input }` — tool invocations.
- `ToolResult { session_id, name, output }` — tool outputs.
- `TurnEnd { session_id, reason }` — turn complete.
- `StateChanged { session_id, state }` — active/idle transitions.
- `Exited { session_id, code }` — child process exited.

Tauri backend subscribes, re-emits as `daemon-event:{session_id}` Tauri events to the frontend, which the new structured view consumes.

## Protocol (JSON-lines over Unix socket)

Every message is one JSON object per line. Both directions.

**Requests (UI → daemon):**

```json
{"type":"spawn","session_id":"...","workdir":"...","kind":"claude","initial_prompt":null,"continue_session":false}
{"type":"deliver","session_id":"...","content":"fix the bug"}
{"type":"stop","session_id":"..."}
{"type":"subscribe","session_ids":["..."]}
{"type":"list"}
{"type":"ping"}
```

**Responses and events (daemon → UI):**

```json
{"type":"ok","request_id":"..."}
{"type":"error","request_id":"...","message":"..."}
{"type":"event","session_id":"...","event":{"kind":"text_delta","text":"..."}}
```

## UI strategy

### Phase 0 (this design)
Document the design, create the tracking issue.

### Phase 1 — Daemon skeleton, no UI changes
- Workspace reorganization: move `src-tauri` into `crates/`, add `controller-core`, `controller-daemon`.
- Daemon boots, accepts connections, can spawn/stop an agent, parses stream-json, broadcasts events.
- Tauri backend has a `DaemonClient` that can connect but isn't wired into any UI code yet.
- No user-visible change. Verified by integration tests that spawn the daemon and drive it over a socket.

### Phase 2 — Parallel path behind a per-session flag
- Add `SessionConfig.communication_mode: "tmux" | "daemon"` (default "tmux").
- New session creation UI lets you pick "daemon" mode.
- New `StructuredTerminal.svelte` component renders daemon events (message list, tool call cards, thinking blocks).
- `TerminalManager.svelte` dispatches to either the existing `Terminal.svelte` (xterm) or `StructuredTerminal.svelte` based on mode.
- Both paths coexist. Terminal path unchanged.

### Phase 3 — Dogfood and iterate
- Switch default to "daemon" for new sessions.
- Migrate features that depend on PTY I/O today (hotkeys, paste, summary pane) to their daemon equivalents.
- Validate that status detection, auto-worker, staging, secure-env all work under the daemon path.

### Phase 4 — Retire tmux path
- Once every feature is on the daemon path and we've run on it for a while, remove `pty_manager.rs`, `tmux.rs`, `status_socket.rs`'s hook responsibilities, and `Terminal.svelte`.
- Keep status_socket for secure-env and stage messages, which are daemon-independent.

## Key modules touched

| Module | Phase | Change |
|--------|-------|--------|
| `Cargo.toml` (root) | 1 | Convert to workspace. |
| `crates/controller-core/` | 1 | New. Shared types. |
| `crates/controller-daemon/` | 1 | New. Binary + lib. |
| `crates/src-tauri/src/daemon_client.rs` | 1 | New. Socket client. |
| `crates/src-tauri/src/commands.rs` | 2 | Route `connect_session`, `write_to_pty` to daemon when mode=daemon. |
| `crates/src-tauri/src/models.rs` | 2 | Add `communication_mode` to `SessionConfig`. |
| `src/lib/StructuredTerminal.svelte` | 2 | New. Structured event renderer. |
| `src/lib/TerminalManager.svelte` | 2 | Dispatch by mode. |
| `src/lib/Terminal.svelte` | 4 | Remove. |
| `src-tauri/src/pty_manager.rs` | 4 | Remove (or keep only for tmux path if we still support it). |
| `src-tauri/src/tmux.rs` | 4 | Remove. |

## Validation

Each phase has its own validation criteria. At design time, the most important tests are:

**Phase 1:**
- Integration test: daemon binary spawns Claude in a temp workdir, receives a text prompt via `deliver`, emits a `text_delta` event, and `turn_end` within N seconds.
- Integration test: daemon survives its socket client disconnecting and reconnecting mid-session.
- Unit tests on stream-json parser for each event type.

**Phase 2:**
- E2E test (Playwright): create a session in daemon mode, send a prompt, see structured output rendered. Validated via `the-controller-general-e2e-eval` skill.
- Both modes coexist: creating a session in tmux mode still works.

**Phase 3:**
- All existing e2e scenarios pass with mode=daemon.

**Phase 4:**
- `cargo test` and `pnpm test` pass after tmux/xterm removal.
- Binary size / startup time regression check.

## Risks & open questions

- **Daemon lifecycle.** Who starts the daemon? Options: (a) Tauri app spawns it as a sidecar on launch and leaves it running; (b) launchd/agent manages it; (c) daemon is spawned on first `connect_session` call. Leaning toward (a) — simplest for local desktop.
- **Daemon upgrades.** If the daemon is running and the Tauri app updates, the running daemon is stale. Need a protocol version field and a "restart daemon" path.
- **Claude Code stream-json stability.** The stream-json format is not a stable API contract. We'll need a compatibility layer and tests that catch format drift early.
- **Input richness.** Today, `send_raw_to_pty` sends CSI-u escape sequences for modifier keys. In a structured world, these don't map — the UI sends structured messages instead. Any feature relying on raw key passthrough needs a new design.
- **Staged sessions.** Current staging spawns a separate Tauri instance bound to a session's socket. Under the daemon model, staging likely just means "open a second UI subscriber for the same agent." Needs design in phase 2.

## Not in this design

- Cloud bridge (slock-style remote collaboration).
- MCP tools for agent↔UI communication. Agents keep using Claude Code's normal MCP set.
- Replacing the hook-based `status_socket` for secure-env and stage messages; those stay.
