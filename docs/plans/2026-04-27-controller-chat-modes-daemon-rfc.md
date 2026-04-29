# RFC: Controller Chat Modes and Daemon

Date: 2026-04-27
Status: Draft

## Source Designs

This RFC consolidates and updates earlier design work:

- `docs/plans/2026-04-22-chat-mode-design.md`
- `~/projects/the-controller-daemon/docs/plans/2026-04-20-daemon-chat-architecture-design.md`
- `~/projects/the-controller-daemon/docs/plans/2026-04-26-adhoc-agent-profiles-design.md`
- `docs/keyboard-modes.md`

Some source docs assume a Tauri app. The current Controller is an Axum + Svelte
web app, so this RFC updates the client boundary: the browser should not talk to
the daemon directly unless the daemon deliberately supports that origin. The
Controller backend should act as the browser-safe gateway to the local daemon.

## Summary

The Controller should treat chat as a first-class workspace, not as terminal
emulation. A long-running local daemon owns agent sessions, normalizes Claude
Code and Codex output into a shared event model, persists session history, and
streams updates to Controller clients. The web app renders those events as chat,
tool calls, approvals, and status changes.

The product centers on project-scoped chat sessions with Claude Code, Codex,
and user-defined agent profiles.

The daemon is the durable session layer. The Controller web app remains the
workspace layer: projects, navigation, keyboard interaction, secure env, issue
views, and visual presentation.

## Use Cases

### Continue Work Without Keeping a Terminal Alive

You start a long-running agent session, close or reload the web app, and come
back later. The session still exists. The transcript reloads from the daemon
event log, and new events stream from the daemon when you reopen the session.

### Switch Between Projects and Agents Quickly

You work across multiple repositories. The sidebar groups chat sessions by
Controller project. Keyboard navigation lets you move through projects and chat
sessions, create a new chat for the focused project, and jump into the active
composer.

### Use Claude Code and Codex Through One Chat Surface

You can create either a Claude Code or Codex session from the same UI. The UI
does not know either runtime's native output format. The daemon parses each
runtime and emits a shared event schema.

### Review Tool Calls in Context

When an agent asks to run a tool, the chat transcript shows the call, its input,
its result, and any required approval. You approve or deny in the transcript
without losing the surrounding conversation.

### Recover From App, Browser, or Daemon Restarts

You can reload the browser, restart the Controller web server, or restart the
daemon. The daemon keeps persisted events in SQLite. When the daemon itself
restarts, it uses the agent runtime's native resume support when possible and
records structured recovery events in the transcript.

### Create Reusable Agent Profiles

You can register a named agent profile such as "Reviewer" or "Planner" with a
runtime, prompt, and skill set. Starting a profile-backed chat creates a fresh
daemon session folder with the prompt and skills materialized for the runtime.

### Debug With Local Tools

You can inspect the daemon with `curl`, `wscat`, and `sqlite3`. You do not need
to reverse-engineer PTY bytes. Every command and response has a typed shape.

## User Stories

### Text Chat

- As a developer, I want to start a new Claude Code chat for the focused
  project so I can ask for implementation work without leaving the Controller.
- As a developer, I want to start a Codex chat in the same project list so I can
  choose the runtime that fits the task.
- As a developer, I want the transcript to survive browser reloads so I can
  treat the Controller as a workspace, not a fragile terminal.
- As a developer, I want keyboard shortcuts for session selection, new chat,
  composer focus, send, and interrupt so repeated chat use stays fast.
- As a developer, I want tool calls and approvals inline so I can judge the
  request against the transcript that caused it.

### Session Recovery

- As a developer, I want a running agent to remain visible after I reload the
  web app.
- As a developer, I want the daemon to record when a session resumed or failed
  to resume so I can trust what happened.
- As a developer, I want pending tool approvals to wait for me instead of
  timing out while I am away.

### Agent Profiles

- As a developer, I want to register a reusable agent with a prompt and skill
  list so I do not rebuild the same setup for every chat.
- As a developer, I want profile edits to affect future sessions only so a live
  session does not change under me.
- As a developer, I want raw Claude and Codex sessions to keep working for
  ad hoc work.

### Operations

- As a developer, I want the Controller to start or reconnect to the daemon for
  me so chat mode works without a separate terminal command.
- As a developer, I want clear daemon status when the daemon is missing,
  locked, unauthorized, or incompatible.
- As a developer, I want local-only auth with a token file so another process
  on the machine cannot call the daemon by accident.

## Product Requirements

1. The Controller exposes chat as a workspace mode.
2. Chat sessions are project-scoped by default.
3. The daemon owns agent processes and conversation state.
4. The web app renders typed events, not terminal bytes.
5. Claude Code and Codex share one UI event model.
6. Tool calls, tool results, approvals, errors, status changes, and token usage
   have durable event records.
7. Browser reloads and Controller server restarts do not lose chat history.
8. Daemon restarts preserve history and attempt native agent resume.
9. The system remains inspectable with local command-line tools.

## Non-goals

- Remote multi-user access in v1.
- Windows support in v1.
- Arbitrary executable agent runtimes.
- Moving project storage, worktree management, or secure env ownership into the
  daemon.
- Recovering an in-flight partial answer after daemon crash. The daemon records
  the interruption and relies on native runtime resume for the next turn.
- Rebuilding already-running profile-backed sessions after a profile edit.

## Concepts

### Controller Web App

The web app owns workspace presentation:

- project list and project focus;
- workspace modes: Chat, Agents, and Kanban;
- keyboard navigation and help;
- secure env prompts;
- issue and maintainer views.

The app should not own long-running agent subprocesses.

### Controller Backend

The Axum backend owns browser-safe local capabilities:

- project storage;
- secure local file reads such as the daemon token;
- a daemon gateway for HTTP and WebSocket traffic;
- optional daemon lifecycle management.

Because the current UI runs in a browser, the backend should shield the browser
from daemon auth and CORS details.

### Daemon

The daemon owns chat runtime state:

- agent process supervision;
- session rows;
- event log;
- native runtime resume;
- agent profile materialization;
- HTTP and WebSocket API.

The daemon binds to localhost and stores state under `~/.the-controller/`.

### Agent Runtime

An agent runtime is the low-level execution engine: Claude Code or Codex. The
daemon chooses the binary, argument shape, stdin encoder, stdout parser, and
resume command for each runtime.

### Agent Profile

An agent profile is a user-defined wrapper around a runtime. It names the
agent's purpose, prompt, skills, and selected runtime. Profiles create sessions
through the same daemon supervisor path as raw runtime sessions.

## Proposed Architecture

### High-level Shape

```text
+-----------------------+      same-origin HTTP/WS      +----------------------+
| Browser UI           | <---------------------------> | Controller backend   |
| Svelte chat modes    |                              | Axum gateway         |
+-----------------------+                              +----------+-----------+
                                                                  |
                                                                  | localhost
                                                                  | token auth
                                                                  v
                                                       +----------+-----------+
                                                       | the-controller-daemon|
                                                       | sessions + events    |
                                                       +----------+-----------+
                                                                  |
                                                                  | stdio JSON
                                                                  v
                                                       +----------+-----------+
                                                       | Claude Code / Codex  |
                                                       +----------------------+
```

The browser talks to the Controller backend on one origin. The backend proxies
daemon HTTP and WebSocket calls or exposes narrow Controller-shaped endpoints.
The backend reads the daemon token from disk and keeps that token out of browser
JavaScript.

This updates the older frontend-direct design. The older design fit a Tauri
webview because the client was trusted and CORS did not matter. The current web
app needs a gateway.

### Daemon API

The daemon keeps the HTTP and WebSocket API from the daemon architecture design:

```text
POST   /sessions
GET    /sessions
GET    /sessions/:id
DELETE /sessions/:id
POST   /sessions/:id/messages
GET    /sessions/:id/messages
GET    /sessions/:id/stream
POST   /sessions/:id/resume

GET    /skills
POST   /skills
GET    /skills/:id
DELETE /skills/:id

GET    /agents
POST   /agents
GET    /agents/:id
PATCH  /agents/:id
DELETE /agents/:id
POST   /agents/:id/sessions
```

The Controller backend can expose these routes under a same-origin prefix such
as `/api/daemon/...`, or it can wrap them in frontend-specific commands. The RFC
prefers a narrow gateway first: proxy the daemon API shape with auth handled by
the backend, then add Controller-specific helpers only where the UI needs them.

### Authentication

The daemon generates `~/.the-controller/daemon.token` on first start with mode
`0600`. The Controller backend reads it and sends `Authorization: Bearer <token>`
to the daemon.

The browser never receives the raw daemon token. This avoids token leakage in
frontend logs, browser devtools, and WebSocket URLs.

### Lifecycle

Target behavior:

1. The Controller backend checks whether the daemon is running when chat mode
   first needs it.
2. If no daemon listens on the configured port, the backend starts it.
3. If a lockfile shows another daemon already running, the backend connects to
   the existing process.
4. If startup fails, chat mode shows a concrete status with the failing command
   and relevant stderr tail.

Manual daemon startup remains useful in development, but the product should not
require a separate terminal command.

### Session Storage

The daemon stores sessions and events in SQLite:

- `sessions`: one row per daemon-owned session;
- `events`: one ordered per-session log containing `inbox`, `outbox`, and
  `system` channels;
- profile tables and skill-library tables for user-defined agents.

The daemon writes to SQLite before it publishes events to WebSocket subscribers.
Any event a client sees live must also be replayable after reconnect.

### Event Model

Every event has:

```text
session_id
seq
channel
kind
payload
created_at
applied_at
```

Channels:

- `inbox`: client commands such as `user_text`, `interrupt`, and
  `tool_approval`.
- `outbox`: agent output such as text, tool calls, tool results, token usage,
  and parser errors.
- `system`: daemon-observed events such as session start, resume, interruption,
  crash, and status changes.

The UI reducer treats `seq` as the dedupe key. The daemon treats SQLite as the
serialization point.

### Streaming

The daemon streams session events over WebSocket:

```text
GET /sessions/:id/stream?since=<last_seq>&channels=...
```

On connect, the daemon reads persisted events where `seq > since`, sends that
gap, then switches to the live broadcast bus. Live token deltas may stay
transient, but every finalized message must appear as a durable `agent_text`
event.

### Agent Supervision

Each live session gets one supervisor task group:

- dispatcher: reads unapplied inbox rows and writes JSON commands to agent
  stdin;
- reader: reads stdout, parses runtime-native JSON, appends normalized events,
  and publishes to the bus;
- exit watcher: records crash or clean exit and updates session status.

The daemon never sends raw PTY bytes to the UI.

### Interrupts

The UI sends an `interrupt` inbox command. The daemon handles the escalation:

1. send the runtime-native cancel message if the runtime supports one;
2. wait briefly;
3. send `SIGINT`;
4. wait briefly;
5. send `SIGTERM`.

The chat input should keep plain `Esc` for leaving the composer and use
`Shift+Esc` for interrupt. This gives each key one job.

## Chat Modes

### Text Chat Mode

Text chat mode is the default chat surface.

It includes:

- project-grouped session list;
- new-chat flow for Claude Code, Codex, and later agent profiles;
- transcript renderer for user messages, agent text, thinking, tool calls,
  tool results, approvals, errors, and status lines;
- composer with `Cmd/Ctrl+Enter` send and `Shift+Esc` interrupt;
- keyboard navigation from `docs/keyboard-modes.md`.

The text chat UI should render the daemon event stream. It should not depend on
agent-specific stdout details.

### Agent Profile Chat

Agent profile chat lets the user spawn a named, reusable agent.

The daemon owns:

- skill library registration;
- profile CRUD;
- profile validation;
- session-folder materialization;
- runtime selection;
- recovery from the materialized session folder.

The Controller UI owns:

- profile editor;
- skill picker;
- profile-backed "New chat" choices;
- profile labels in the session list.

Raw `POST /sessions` remains available for direct Claude and Codex sessions.

## UI and Navigation

The Controller keeps the current workspace-mode model:

```text
Space -> a  Agents
Space -> k  Kanban
Space -> c  Chat
```

Chat shortcuts:

```text
j / k                 move through visible project and chat rows
l / Enter             expand project or select chat
n                     create chat for focused project
i                     focus active composer
Cmd/Ctrl+Enter        send from composer
Esc in composer       leave composer
Shift+Esc in composer interrupt active turn
```

The help overlay should derive these from the command registry where possible.
Input-local keys should live in the chat documentation and input placeholder.

## Error Handling

### Daemon Unavailable

The UI should distinguish these cases:

- daemon not installed;
- daemon not running;
- daemon startup failed;
- token missing or unreadable;
- token rejected;
- daemon reachable but incompatible;
- browser gateway or WebSocket proxy failed.

The current "Daemon not running" state is too broad for the target product.

### Session Missing

If a session disappears out of band, the UI removes it from the store, clears
active selection if needed, and shows a short error.

### Ended Sessions

Ended and failed sessions keep their transcripts. Inbox commands return `409`.
The composer disables itself and offers a new-chat path.

### Tool Approvals

Tool approval requests do not time out by default. When the user reconnects,
the transcript still shows the pending tool call and approval controls.

### Browser Boundary Failures

Because the current app runs in a browser, the gateway must handle CORS and
WebSocket upgrade details. The browser should never make authenticated
cross-origin calls to the daemon in the default path.

## Migration Plan

### Phase 1: Stabilize Text Chat on the Daemon

- Keep the current Chat workspace.
- Move browser-to-daemon traffic behind the Controller backend gateway.
- Improve daemon status reporting.
- Keep manual daemon startup in dev, but add backend-managed startup for normal
  use.
- Preserve the existing project-scoped session list and keyboard model.

### Phase 2: Complete Event Rendering

- Fill gaps in transcript rendering: system lines, errors, token usage,
  reconnect status, and long transcripts.
- Keep tool calls collapsed by default.
- Add integration tests with the daemon fake agent.

### Phase 3: Agent Profiles

- Add daemon skill and profile APIs.
- Add UI for profile CRUD.
- Add profile-backed "New chat" choices.
- Validate recovery from materialized profile sessions.

### Phase 4: Optional Remote Clients

- Keep localhost as the default.
- Revisit token scope, transport security, and multi-client policy before
  exposing the daemon beyond the local machine.

## Testing Strategy

### Daemon

- Unit-test event storage, auth, validators, parsers, and profile
  materialization.
- Integration-test the HTTP and WebSocket surface with the fake agent.
- Test crash, resume, interrupt, malformed output, and replay without
  duplicates.
- Keep real Claude and Codex smoke tests opt-in.

### Controller Backend

- Test daemon token read without leaking token values in output.
- Test gateway HTTP proxy behavior.
- Test WebSocket proxy replay and close behavior.
- Test daemon lifecycle handling: not installed, not running, already running,
  startup failure.

### Frontend

- Unit-test command registry and mode-specific keymaps.
- Component-test chat input, transcript blocks, tool approvals, new-chat modal,
  daemon empty states, and profile choices.
- E2E-test a full text chat against the fake daemon: create, send, stream,
  approve tool call, reload browser, and reconnect without duplicates.

## Open Questions

1. Should the Controller backend expose a generic daemon proxy or a smaller
   Controller-shaped API?
2. Should the daemon emit live deltas for thinking blocks, or only finalized
   thinking?
3. How should the UI show token usage: per message, session footer, or both?
4. What compatibility policy do we need when the daemon schema version and
   Controller frontend version differ?
5. Should backend-managed daemon startup use a bundled release binary, `cargo
   run` in development, or a user-configured path?

## Decision Summary

- Use the daemon as the durable chat session layer.
- Use the Controller backend as the browser-safe gateway to the daemon.
- Keep the web app responsible for project UX, workspace modes, keyboard
  navigation, and presentation.
- Share one daemon event model across Claude Code, Codex, agent profiles, and
  text chat.
