# Chat mode — design

Date: 2026-04-22
Status: Approved, ready for implementation planning

## Context

`the-controller-daemon` (separate Rust project at `~/projects/the-controller-daemon/`) is a long-running local daemon that owns Claude Code and Codex agent sessions and exposes them through an HTTP + WebSocket API with a typed event schema (channels: `inbox`, `outbox`, `system`). Sessions, inbox commands, and outbox events are persisted to SQLite; clients can disconnect and reconnect with `?since=<seq>`.

The daemon's own design doc (`~/projects/the-controller-daemon/docs/plans/2026-04-20-daemon-chat-architecture-design.md`) explicitly defers "Tauri app integration (new `development-v2` UI mode built on top of the daemon)" as follow-up work. This project is that follow-up: a new `chat` workspace mode in the Tauri app that renders daemon sessions as structured messages.

## Scope

**In scope:**

- New workspace mode `chat` alongside `development` and `agents`.
- Full multi-session workspace: session list, create / pick / delete daemon sessions, tied to existing Tauri `Project` entities via `cwd`.
- Support both Claude and Codex from day one.
- Live streaming of `agent_text_delta`; finalized on `agent_text`.
- Tool calls rendered inline, collapsed by default.
- Tool approval via inline Approve / Deny buttons in the transcript.
- Manual daemon lifecycle — the Tauri app assumes the user runs `the-controller-daemon` externally. Disconnected state shows an empty-state with retry.
- Coexist with `development` mode. No migration / deprecation here.

**Out of scope:**

- Auto-spawning the daemon from the Tauri app (revisit later).
- Sunsetting the `development` (terminal) mode.
- Remote daemon access.
- Load testing many concurrent sessions.
- Arbitrary-cwd sessions independent of projects (project-scoped only in v1).

## Architecture overview

The Svelte frontend talks to the daemon directly over HTTP + WebSocket (Approach A). One thin Tauri command `read_daemon_token() -> Result<String>` reads `~/.the-controller/daemon.token`. Everything else — session CRUD, message send, WS subscription with replay — lives in Svelte.

```
┌──────────────────────────────┐    HTTP + WS     ┌──────────────────────────┐
│  Tauri app (chat mode)       │ ───────────────► │  the-controller-daemon   │
│                              │  localhost +     │  (Rust, long-lived)      │
│  src/lib/daemon/             │   bearer token   │                          │
│    client.ts  stream.ts      │                  │                          │
│    types.ts   store.ts       │                  │                          │
│                              │                  │                          │
│  Tauri cmd: read_daemon_token│                  │                          │
└──────────────────────────────┘                  └──────────────────────────┘
                                                        │
                                                        ├── ~/.the-controller/daemon.db
                                                        ├── ~/.the-controller/daemon.token
                                                        └── ~/.the-controller/daemon.pid
```

### Why Approach A (frontend-direct)

- The daemon's HTTP+WS is already typed, debuggable, and stable; a Rust shim in `src-tauri` would just be passthrough.
- Tauri webview is a trusted client; bearer token in JS memory is acceptable.
- Fastest path to shipping a working chat UI.
- If we later need remote daemons or a central reconnection manager, we migrate the hot path into Rust then. The Svelte-side types, store, and UI don't need to change.

Approaches B (Rust owns everything) and C (hybrid: Rust owns WS only) were considered and rejected for v1 on cost / velocity grounds.

## Workspace mode integration

The app's current mode switcher is `Space` → mode key (`HotkeyManager.svelte:87-103`, `WorkspaceModePicker.svelte:8-11`). We add a third mode:

- `WorkspaceMode` type (`src/lib/stores.ts:131`) gains `"chat"`.
- `WorkspaceModePicker.svelte` adds `{ key: "c", id: "chat", label: "Chat" }`.
- `HotkeyManager.svelte:handleWorkspaceModeKey` gains a branch for `"c"` → `workspaceMode.set("chat")`.
- `App.svelte:375-379` grows a third branch: `{:else if workspaceModeState.current === "chat"} <ChatWorkspace /> {:else} <TerminalManager /> {/if}` (`development` stays as the default `:else`).
- `HotkeyHelp.svelte` grows a case for chat-mode hotkeys.
- `focusForModeSwitch` gains a `chat` branch — focus the active chat session, else the first project.

## UI layout

Three-pane layout, reusing the existing sidebar.

```
┌─────────────────────────────────────────────────────────────────────┐
│ Existing Sidebar (projects)                                          │
│ ┌─────────────────┐ ┌────────────────────────────────────────────┐   │
│ │ Project tree    │ │ ChatView                                   │   │
│ │ ── project A    │ │  ┌──────────────────────────────────────┐  │   │
│ │    ├─ chat 1    │ │  │ header: label · agent · status       │  │   │
│ │    └─ chat 2    │ │  ├──────────────────────────────────────┤  │   │
│ │ ── project B    │ │  │ transcript                           │  │   │
│ │    └─ + New chat│ │  │  user bubble                         │  │   │
│ │                 │ │  │  agent bubble (markdown, deltas)     │  │   │
│ │                 │ │  │  🔧 tool_call ▸  (click to expand)   │  │   │
│ │                 │ │  │  [approve] [deny] (pending approval) │  │   │
│ │                 │ │  ├──────────────────────────────────────┤  │   │
│ │                 │ │  │ ChatInput (Cmd+Enter send, Esc intr) │  │   │
│ │                 │ │  └──────────────────────────────────────┘  │   │
│ └─────────────────┘ └────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

### New components (`src/lib/chat/`)

- `ChatWorkspace.svelte` — root mounted when `workspaceMode === "chat"`. Owns Sidebar + ChatView composition for chat mode; handles empty states (no daemon, no projects, no sessions).
- `ChatSessionList.svelte` — rendered inside `Sidebar.svelte` when in chat mode (parallel to the terminal session list). Shows daemon sessions grouped by project, with status dots. Includes "+ New chat" per project.
- `NewChatDialog.svelte` — modal opened from "+ New chat". Agent dropdown (`claude` | `codex`), optional initial prompt, confirm. `cwd` is inherited from the project's main worktree.
- `ChatView.svelte` — header + transcript + input. Subscribes to one session's WS stream on mount; tears down on unmount / session switch.
- `Transcript.svelte` — virtualized list (reuse existing virtualization if present, else plain scroll container for v1). Renders logical blocks.
- `MessageBlock.svelte` — dispatches on event kind to subcomponents: `UserMessage`, `AgentMessage` (markdown, in-progress indicator for deltas), `ThinkingBlock`, `ToolCallBlock` (collapsed by default; expanded shows input + paired `tool_result`), `ErrorBlock`, `StatusLine`.
- `ChatInput.svelte` — textarea, `Cmd+Enter` sends, `Esc` interrupts when agent is running, disabled on `ended` / `failed`.
- `DaemonEmptyState.svelte` — shown when token read or HTTP ping fails. Retry button.

### Sidebar + navigation

`Sidebar.svelte` already branches on `currentMode` (line 27). Add a third branch for `chat` rendering `ChatSessionList`. The keyboard nav shape from `HotkeyManager.svelte:getVisibleItems` extends to chat items (project → chats → next project). `Enter` on a chat activates it; `Enter` on "+ New chat" opens `NewChatDialog`.

## Data flow

### Send a user message

```
ChatInput → store.sendUserText(sid, text)
  client.post(`/sessions/${sid}/messages`, {kind:"user_text", text})
  → 202 {seq}
  → store optimistic-inserts an inbox event at that seq
  → WS delivers the same event; dedupe on (sid, seq)
```

### Receive agent output

```
stream.ts WS for active session
  outbox:agent_text_delta  → append to inProgressBlocks[block_id]
  outbox:agent_text        → drop inProgress for block_id; finalize
  outbox:tool_call         → render collapsed block (awaiting tool_result by call_id)
  outbox:tool_result       → attach to its tool_call at render time
  outbox:token_usage       → update transcript.tokenUsage
  outbox:error             → inline error block
  system:status_changed    → update transcript.statusState
  system:session_ended     → disable input; show ended banner
```

### Reconnect

```
WS close (any reason) → exponential backoff cap 10s
  reconnect with ?since=<lastSeq>
  daemon replays seq > lastSeq (finalized only; no deltas) then switches live
  any partial inProgressBlocks are dropped; finalized replay fills the gap
```

## Types

Mirrors `the-controller-daemon/src/model.rs` exactly in `src/lib/daemon/types.ts`.

```ts
export type Agent = "claude" | "codex";
export type SessionStatus = "starting" | "running" | "interrupted" | "ended" | "failed";
export type Channel = "inbox" | "outbox" | "system";

export interface DaemonSession {
  id: string;
  label: string;
  agent: Agent;
  cwd: string;
  args: string[];
  status: SessionStatus;
  native_session_id: string | null;
  pid: number | null;
  created_at: number;
  updated_at: number;
  ended_at: number | null;
  end_reason: string | null;
}

export interface EventRecord {
  session_id: string;
  seq: number;
  channel: Channel;
  kind: string;
  payload: unknown;
  created_at: number;
  applied_at: number | null;
}

export type OutboxEvent =
  | { kind: "agent_text"; payload: { message_id: string; block_id: string; text: string; role?: string } }
  | { kind: "agent_text_delta"; payload: { message_id: string; block_id: string; delta: string; role?: string } }
  | { kind: "agent_thinking"; payload: { message_id: string; block_id: string; text: string } }
  | { kind: "tool_call"; payload: { call_id: string; tool: string; input: unknown } }
  | { kind: "tool_result"; payload: { call_id: string; output: unknown; is_error: boolean } }
  | { kind: "token_usage"; payload: { input: number; output: number; cache_read: number; cache_write: number } }
  | { kind: "error"; payload: { code: string; message: string; detail?: unknown } };

export type InboxEvent =
  | { kind: "user_text"; payload: { text: string } }
  | { kind: "interrupt"; payload: {} }
  | { kind: "tool_approval"; payload: { call_id: string; approved: boolean; reason?: string } };

export type SystemEvent =
  | { kind: "session_started"; payload: { agent: Agent; cwd: string; args: string[] } }
  | { kind: "session_ended"; payload: { end_reason: string; exit_code?: number; signal?: string } }
  | { kind: "session_interrupted"; payload: { reason: string } }
  | { kind: "session_resumed"; payload: { native_session_id: string } }
  | { kind: "agent_crashed"; payload: { exit_code?: number; signal?: string; last_stderr_tail?: string } }
  | { kind: "status_changed"; payload: { state: "starting"|"idle"|"working"|"waiting_for_tool_approval"|"failed"; idle_ms?: number } };
```

## Store shape

Module-level Svelte 5 `$state` in `src/lib/daemon/store.ts`.

```ts
interface DaemonState {
  token: string | null;
  reachable: boolean;
  sessions: Map<string, DaemonSession>;
  transcripts: Map<string, TranscriptState>;
  streams: Map<string, StreamState>;
}

interface TranscriptState {
  events: EventRecord[];           // finalized, ordered by seq
  lastSeq: number;
  inProgressBlocks: Map<string, string>;  // block_id -> accumulated delta text
  tokenUsage: OutboxEvent["payload"] | null;
  statusState: SystemEvent["payload"]["state"] | null;
}

interface StreamState {
  ws: WebSocket | null;
  connected: boolean;
  reconnectAt: number | null;
}
```

**Key properties:**

- `Transcript.svelte` folds `inProgressBlocks` onto the tail at render time — deltas show as a partial agent block; `agent_text` finalize drops the partial.
- `tool_result` is joined to its `tool_call` by `call_id` at render time (flat event log matches the daemon).
- Dedupe: every incoming event keyed by `(sid, seq)`. Optimistic local insert on `POST /messages` uses the 202 `seq`; WS delivery of the same event is a no-op.
- Store is module-scoped → state survives mode switches. WS is kept open only for the active session; inactive sessions rebuild with `?since=<lastSeq>` on reopen.
- Project↔daemon-session mapping: `DaemonSession.cwd === project.path` (exact string). Sessions with no matching project land in an "Other" group (not hidden; users may create sessions via `curl`).

## Error handling

### Daemon unreachable

**Triggers:** token file missing or unreadable; HTTP connect refused; WS connect fails before any session is open.

**UX:** `ChatWorkspace` renders `DaemonEmptyState` full-panel — title "Daemon not running", start command shown (`./target/release/the-controller-daemon`), Retry button.

**Behavior:** ping = `GET /sessions`. On mount and on Retry. No automatic polling.

### HTTP errors mid-session

| Status | Response |
|---|---|
| 401 | Re-read token once, retry once. Still 401 → `reachable = false`, render `DaemonEmptyState`. |
| 404 | Session deleted out-of-band. Remove from store, close WS, toast "Session no longer exists." |
| 409 | Session ended. Disable input; banner "Session ended. Start a new one." Transcript preserved. |
| 422 | Log + toast "Invalid request — please report." Input stays enabled. |
| 503 | Toast "Daemon storage error." Input stays enabled. |
| Network error | Inline retry icon next to input. No auto-retry — duplicate delivery worse than visible failure. |

### WS disconnection

- Exponential backoff cap 10s, reconnect with `?since=<lastSeq>`.
- Header indicator "Reconnecting…"; at cap, "Disconnected — retry" with manual button.
- `1011 bus_lagged` → same reconnect path.
- On reconnect: partial `inProgressBlocks` are dropped; finalized replay fills them.

### Agent-side errors (outbox / system)

- `error` outbox event → red inline block in transcript (monospace code + message).
- `agent_crashed` system event → `ErrorBlock` with exit code / signal / last stderr tail (collapsed, expandable). Input disabled; status indicator red.
- `session_ended` with `end_reason = "resume_failed"` → neutral system block. Input disabled.
- `session_interrupted` → neutral block. If followed by `session_resumed` shortly, collapse into a single "Reconnected" note.

### Tool approval

No client timeout. Inline Approve / Deny remain present while `status_changed.state === "waiting_for_tool_approval"`.

### Interrupt

UI sends the inbox command; daemon handles the escalation ladder internally. If no `status_changed` in ~5s, non-blocking hint: "Interrupt sent — agent may still be finishing."

## Testing strategy

### Tier 1 — unit (Vitest)

- **Event reducer:** sequence of events → derived transcript matches fixture. Covers delta accumulation, finalize-drops-in-progress, tool_result join by call_id, dedupe on repeated `(sid, seq)`.
- **Types round-trip:** JSON fixtures from `the-controller-daemon/tests/fixtures/` parse and narrow correctly. Schema-drift guard.
- **Sidebar grouping:** projects + sessions → expected grouped structure plus "Other".

### Tier 2 — component (@testing-library/svelte)

- `ChatInput`: `Cmd+Enter` sends; `Esc` interrupts when running, no-op when idle; disabled on `ended`.
- `MessageBlock`: each kind renders without throwing; `ToolCallBlock` toggles.
- `NewChatDialog`: submit disabled until valid; submit fires expected body.
- `DaemonEmptyState`: Retry calls `ping()`; on success, swaps out.

### Tier 3 — integration against the daemon's fake agent (spine)

Reuse `the-controller-daemon/tests/support/fake_agent` (scriptable stream-json binary):

1. `pnpm test:integration` starts `the-controller-daemon` with `TCD_AGENT_CLAUDE_BINARY` → fake agent, `TCD_STATE_DIR` → tmpdir.
2. Playwright test (reuses existing e2e harness):
   - Switch to chat mode, create chat in a test project.
   - Send "hello" → assert user bubble, then streamed deltas, then finalized agent bubble.
   - Assert `tool_call` renders collapsed; expand attaches `tool_result`.
   - Kill daemon → assert `DaemonEmptyState`; restart + Retry → workspace returns.
   - Reconnect with `?since` exercised when WS alone is dropped (daemon alive); assert no duplicates.

### Tier 4 — real-Claude smoke (opt-in, local)

Gated by `CHAT_SMOKE=1`. Not in CI.

### Validation mapping (per CLAUDE.md)

| Change | Test that fails if reverted |
|---|---|
| Register `chat` workspace mode | Playwright: "Space c switches to chat mode" |
| Event reducer | Unit: golden fixture |
| Delta streaming + finalize | Playwright: delta appears then resolves |
| Tool call inline collapsed + expand | Component: `ToolCallBlock` toggle |
| Inline Approve/Deny | Playwright with fake_agent emitting `waiting_for_tool_approval` |
| Reconnect with `?since` | Integration: kill WS mid-stream, assert no duplicates |
| `DaemonEmptyState` on connect failure | Component: ping fails → empty state |
| Group sessions by project cwd | Unit: grouping function |

## Open questions

None blocking. To revisit:

- Exact virtualization strategy if transcripts grow large (start with plain scroll; measure before adding a dep).
- Whether to surface token_usage per-message, footer, or both.
- Whether Codex end-to-end needs a second integration pass (Claude is the primary target).

## Next step

Invoke the `the-controller-writing-plans` skill to produce a detailed implementation plan against this design.
