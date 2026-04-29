# PRD: Chat Routing, Agents, and Workspaces

Date: 2026-04-29
Status: Draft

Related docs:

- `docs/plans/2026-04-27-controller-chat-modes-daemon-rfc.md`
- `docs/plans/2026-04-22-chat-mode-design.md`
- `docs/keyboard-modes.md`

## Table of Contents

- [Summary](#summary)
- [Design Assets](#design-assets)
- [Problem](#problem)
- [Goals](#goals)
- [Non-goals](#non-goals)
- [Users](#users)
- [Use Cases](#use-cases)
- [User Stories](#user-stories)
- [Product Principles](#product-principles)
- [Frontend UX Requirements](#frontend-ux-requirements)
- [Frontend UI Requirements](#frontend-ui-requirements)
- [Backend and Daemon Requirements](#backend-and-daemon-requirements)
- [Lifecycle Rules](#lifecycle-rules)
- [Error States](#error-states)
- [Acceptance Criteria](#acceptance-criteria)
- [Metrics](#metrics)
- [Decision Log](#decision-log)
- [Proposed Phases](#proposed-phases)

## Summary

The Controller chat surface should become a routing workspace. A user starts a
chat first, writes naturally, and then routes work to agents and workspaces from
inside the composer.

The main product shift is that new chat creation should not begin with an agent
selection modal. Pressing `n` should create a new chat, focus the composer, and
let the user decide who or what to involve while they type.

Chat routing has three core controls:

- `@agent` targets a reusable long-running agent session and sets persistent
  agent focus.
- `%agent` queues a new shadow agent session that spawns when the message is
  sent.
- `w` fuzzy-finds a workspace, associates it with the chat, and spawns an agent
  worktree.

The daemon remains responsible for durable agent sessions, process supervision,
event streaming, and ownership cleanup. The frontend is responsible for the
keyboard-first composition flow, token suggestions, summary pane, and clear
agent/workspace association.

## Design Assets

- [Chat Routing Mode mockup](../assets/design/controller-chat-routing-ui.png)

## Problem

The current chat creation flow asks the user to choose an agent before they have
started thinking in the chat. That makes chat feel like a launcher rather than a
workspace.

The emerging product need is different:

- Users want to create a chat instantly and start typing.
- Users want to call an existing long-running agent without binding that agent
  to the chat lifecycle.
- Users want to spawn a fresh shadow agent that belongs to the chat and is
  cleaned up with it.
- Users want to scope a chat to one or more workspace worktrees without leaving
  the chat surface.
- Users want workspace rows in the sidebar so they can start chats directly
  under a workspace.
- Users want a dedicated chats section in the sidebar for agent-chat work that
  does not begin from a specific workspace row.
- Users need lightweight context at the top of the chat showing every agent and
  workspace associated with the current chat, including idle ones.

## Goals

1. Make new chat creation immediate from chat mode.
2. Let users route messages to configured agents from the composer.
3. Clearly distinguish reusable agents from shadow agents.
4. Let users fuzzy-find, create, and focus worktree-backed workspaces from chat.
5. Show every agent and workspace associated with the current chat in a compact
   summary pane.
6. Keep keyboard behavior single-purpose and predictable.
7. Give the daemon enough ownership metadata to clean up shadow sessions.
8. Keep the system durable across browser reloads and daemon reconnects.
9. Let sidebar focus determine whether `c` creates a workspace-scoped chat or a
   general agent chat.

## Non-goals

- Remote multi-user collaboration.
- Replacing the existing Agents or Kanban workspace modes.
- Full project management for worktrees outside the chat workflow.
- Arbitrary runtime plugins beyond configured Controller agent profiles.
- Non-text input modes.
- Finalizing every persistence schema detail in this PRD.

## Users

### Interactive Developer

Works inside the Controller as a daily command center. They want to create a
chat, ask a question, involve the right agent, and stay in keyboard flow.

### Agent Orchestrator

Coordinates multiple agents across review, planning, implementation, and
debugging. They need to reuse long-running agents when useful and spawn
short-lived shadow agents when the work should stay attached to one
conversation.

### Worktree-heavy Developer

Uses isolated git worktrees for experiments, fixes, and reviewable branches.
They need chats and agents to know which workspace they are operating in.

## Use Cases

### Start Writing First

The user presses `n` in chat mode. The Controller creates a new chat for the
focused project and moves focus into the composer. The user starts typing
without picking an agent first.

### Leave the Composer Without Interrupting

The user presses `Esc` while the composer is focused. The composer loses focus
and the new chat row becomes focused in the sidebar. No agent turn is
interrupted.

### Interrupt an Active Turn

The user presses `Shift+Esc` while an active chat turn is running. The
Controller sends an interrupt command for the focused active agent session.

### Call a Reusable Agent

The user types `@reviewer` in the composer. The UI suggests configured agents.
When the user selects `reviewer`, the Controller associates that reusable agent
session with the chat and delivers the current message to its inbox. If the
session is not running yet, the daemon spawns it. The session is not owned by the
chat and is not deleted when the chat is deleted. Selecting `@reviewer` also
sets persistent agent focus. Future chat messages go to every agent that has
been tagged in the chat, including `@reviewer`.

### Spawn a Shadow Agent

The user types `%debugger` in the composer. The UI suggests configured agent
profiles. Selecting `debugger` creates a pending association only. When the user
sends the message, the daemon spawns a new shadow agent session owned by the
current chat. If the chat is deleted, that agent session is interrupted and
despawned.

### Route One Message to Multiple Agents

The user tags multiple agents in one message. Each newly tagged agent becomes
associated with the chat. When the message is sent, every associated agent
receives the message in its inbox. Each agent uses its own context and judgement
to decide whether to publish an outbox reply. Newly tagged agents see the tag in
the message, so they can treat it as a direct invocation.

### Add a Workspace Scope

The user presses `w` in chat mode. The UI opens fuzzy find for workspaces. The
user selects a workspace, the Controller associates it with the chat, and the
backend spawns an agent worktree. Agents do not spawn inside that worktree.
Instead, the daemon updates the agent's prompt and
`AGENTS.md` context with the edit target, then reloads the agent so it knows
which folder to modify.

### Create a Workspace-scoped Chat From the Sidebar

The sidebar can show multiple workspaces. When the user focuses a workspace row
and presses `c`, the Controller creates a new chat under that workspace, spawns
an agent worktree, and focuses the new chat composer.

### Create an Agent Chat From the Sidebar

The sidebar has a `Chats` section for general agent chats. When the user focuses
that section and presses `c`, the Controller creates a new chat in that section
and focuses the composer.

### Track Current Focus

The user looks at the summary pane above the transcript and sees all agents and
workspaces associated with the current chat, including idle ones.

## User Stories

- As a developer, I want `n` to create a chat and focus the composer so I can
  start writing immediately.
- As a developer, I want `Esc` in the composer to return focus to the sidebar so
  I can navigate without stopping a running turn.
- As a developer, I want `Shift+Esc` to interrupt an active turn so interrupt
  has a distinct shortcut.
- As a developer, I want `@agent` suggestions while typing so I can associate a
  reusable agent with the chat without opening a modal.
- As a developer, I want `@agent` to spawn the reusable agent on first use so I
  do not need a separate preflight step.
- As a developer, I want `%agent` to create a shadow agent so one conversation
  can have its own temporary agent.
- As a developer, I want one message to tag multiple agents so I can bring
  several specialists into the chat at once.
- As a developer, I want associated agents to receive each chat message in their
  inbox so they can track the conversation and decide whether to reply.
- As a developer, I want agent replies to come through an explicit outbox so the
  transcript shows intentional responses, not raw process output.
- As a developer, I want shadow agents to be cleaned up when I delete the
  chat so local agent processes do not linger.
- As a developer, I want `w` to fuzzy-find and associate worktree-backed
  workspaces so agents can make edits in the right checkout.
- As a developer, I want workspace rows in the sidebar so I can press `c` on a
  workspace and create a workspace-scoped chat there.
- As a developer, I want a `Chats` section in the sidebar so I can press `c`
  there and create a general agent chat.
- As a developer, I want deleting a chat to delete its worktrees so cleanup is
  clear and local checkouts do not pile up.
- As a developer, I want the summary pane to show every associated agent and
  workspace so I can see the whole chat context at a glance.
- As a developer, I want chats, agents, and workspaces to survive browser
  reloads where their lifecycle says they should.

## Product Principles

### Chat Starts Empty and Useful

The user should not have to classify the work before they can write. A new chat
is a place to think first and route second.

### Routing Belongs in the Composer

Agent and workspace selection should happen close to the message text. The
composer should understand routing tokens instead of forcing the user through
separate dialogs.

### Ownership Must Be Visible

`@agent` and `%agent` may both route work to agents, but they mean different
things. The UI and daemon must preserve that distinction.

### Replies Are Explicit

The Controller should show agent replies that agents choose to publish to their
outbox. Runtime stdout can still exist for supervision and diagnostics, but the
chat transcript should not treat raw stdout as the product reply.

### Keyboard Flow Is a Product Feature

Shortcuts should have one job each. `Esc` leaves the composer. `Shift+Esc`
interrupts. `n` starts a chat from the current chat flow. `c` creates a chat
from the focused sidebar section. `w` scopes work.

## Frontend UX Requirements

### New Chat

- Pressing `n` in chat mode creates a draft chat for the focused project.
- The draft chat becomes the active chat.
- Focus moves into the composer.
- The sidebar shows the new chat row immediately.
- No agent selection modal appears during the default `n` flow.
- Pressing `Esc` in the composer moves focus to the active chat row in the
  sidebar.

### Composer Routing Tokens

- Typing `@` opens reusable agent suggestions.
- Typing `%` opens shadow agent suggestions.
- Suggestions filter as the user types.
- Suggestions use configured agent profiles as their source.
- Selecting a suggestion inserts a stable token into the composer.
- A single message may include multiple agent tokens.
- Tokens should be visually distinct from plain text.
- The composer can contain normal text before and after routing tokens.
- The send path must preserve token identity. It should not rely only on string
  parsing after submission.

### Agent Routing Semantics

- `@agent` associates a reusable agent session with the chat.
- Selecting `@agent` sets persistent agent focus for future messages.
- If the reusable session does not exist, the daemon spawns it when the user
  selects or tags the agent.
- The reusable session is not owned by the chat.
- Deleting the chat does not delete or interrupt reusable sessions created
  through `@agent`.
- `%agent` creates a pending shadow-agent association in the composer.
- The daemon spawns the shadow agent session only when the message is sent.
- Shadow agent sessions record the owning chat id.
- Deleting the chat interrupts and despawns shadow agent sessions.
- One message can tag multiple agents.
- Every agent associated with the chat receives each sent message in its inbox.
- Each associated agent receives its own independent inbox item.
- Newly tagged agents receive the message with their tag preserved.
- Each associated agent decides whether to publish an outbox reply.
- The transcript should show which agent produced each outbox reply.

### Workspace Shortcut

- Pressing `w` in chat mode opens workspace fuzzy find.
- The picker supports selecting an existing workspace.
- Selecting a workspace associates it with the current chat.
- Associating a workspace spawns an agent worktree.
- A chat may have multiple associated workspaces.
- The currently focused workspace determines the default edit target for agent
  work.
- Agents spawn in their own session directories, not inside the focused
  workspace.
- When workspace focus changes, the daemon updates the affected agents'
  prompts and `AGENTS.md` context, then reloads those agents.
- The UI must make it clear when no workspace is focused.

### Summary Pane

- The summary pane appears above the transcript.
- It shows every agent associated with the current chat, including idle agents.
- It shows every workspace associated with the current chat, including idle
  workspaces.
- It marks the focused agent and focused workspace when they exist.
- It supports multiple linked agents and workspaces without becoming a large
  dashboard.
- Empty states should be quiet, not instructional.

Example compact states:

```text
Agent: @reviewer focused        Workspace: controller-chat-routing focused
Agents: @reviewer focused, %debugger idle        Workspaces: controller-chat-routing focused, daemon-api idle
Agent: none        Workspace: none
```

### Keyboard Behavior

| Key | Context | Behavior |
|-----|---------|----------|
| `n` | Chat mode ambient focus | Create chat and focus composer |
| `Esc` | Composer focus | Leave composer and focus active chat row |
| `Shift+Esc` | Composer focus with active turn | Interrupt focused active turn |
| `i` | Chat mode ambient focus | Focus active composer |
| `j` / `k` | Sidebar focus | Move highlight through visible sidebar items |
| `l` | Chat row highlighted | Focus the corresponding chat |
| `w` | Chat mode ambient focus | Fuzzy-find and associate a workspace |
| `c` | Workspace row focused | Create chat under that workspace and spawn an agent worktree |
| `c` | Chats section focused | Create general agent chat |
| `Cmd/Ctrl+Enter` | Composer focus | Send message |

## Frontend UI Requirements

### Chat Shell

- Keep the existing project sidebar as the main navigation anchor.
- The sidebar supports a `Chats` section for general agent chats.
- The sidebar supports multiple workspace rows, each with its own nested chats.
- Replace the new-chat modal with immediate chat creation for the primary flow.
- Preserve a path for explicit advanced chat creation if needed later.
- Show active, focused, running, failed, and ended states in the sidebar.
- Pressing `j` and `k` moves the sidebar highlight through visible sections,
  workspaces, and chat rows.
- Pressing `l` on a highlighted chat row transfers focus to that chat.
- Pressing `l` on a highlighted workspace or section expands, collapses, or
  enters that group according to the existing sidebar pattern.
- Pressing `c` on a workspace row creates a workspace-scoped chat under that
  workspace.
- Pressing `c` on the `Chats` section creates a general agent chat in that
  section.

### Composer

- The composer should support token suggestions without visually dominating the
  transcript.
- Suggestion menus should be keyboard navigable.
- `@` and `%` tokens should be distinguishable by label and styling.
- Deleted or unavailable agents should render as invalid tokens with a clear
  send-time error.

### Transcript

- User messages should preserve newly tagged agents and current agent
  associations.
- Agent responses should identify which agent produced the output.
- Multi-agent turns should render each agent's reply as its own response block.
- The transcript should render only published outbox replies. If an agent
  receives a message and chooses not to reply, the transcript shows no agent
  block for that non-reply.
- The UI should not render placeholder states such as "no outbox reply yet" in
  the transcript.
- Tool calls, tool results, approvals, errors, and status lines continue to
  render from daemon events.
- Shadow agent shutdown caused by chat deletion should produce a system
  event before the chat is removed from the visible list, where practical.

### Workspace Picker

- The picker should fit the current keyboard-first interaction model.
- It should use fuzzy find over known workspaces.
- It should list chat-linked workspaces first when useful.
- Selecting a workspace should associate it with the chat and spawn an agent
  worktree.
- It should show enough path or branch detail to prevent choosing the wrong
  checkout.

## Backend and Daemon Requirements

### Core Entities

```text
Chat
ChatMessage
AgentProfile
AgentSession
ChatAgentLink
Workspace
ChatWorkspaceLink
AgentOutboxItem
```

`Chat` represents the Controller conversation.

`AgentProfile` is a configured agent definition, including runtime, prompt,
skills, and launch settings.

`AgentSession` is a daemon-owned live or resumable process session.

`ChatAgentLink` records that a chat referenced or owns an agent session. It also
tracks focus and idle/running display state for the summary pane.

`Workspace` represents a project worktree or checkout that can be focused by a
chat.

`ChatWorkspaceLink` records workspace association and focus state for a chat.

`AgentOutboxItem` represents an intentional reply or status item an agent has
published for the Controller transcript.

### Agent Session Ownership

Reusable `@agent` sessions:

- are resolved from `AgentProfile`;
- are spawned when tagged if no live session exists;
- have no `owner_chat_id`;
- set persistent focus for future messages in the chat;
- can be referenced by many chats;
- survive chat deletion.

Shadow `%agent` sessions:

- are resolved from `AgentProfile`;
- remain pending until the user sends the message;
- spawn as a new `AgentSession` when the message is sent;
- store `owner_chat_id`;
- are associated with the creating chat;
- are interrupted and despawned when the owning chat is deleted.

Multi-agent routing:

- allows one message to associate multiple reusable and shadow agents with
  the chat;
- delivers each sent chat message to every associated agent inbox;
- creates one independent inbox item per associated agent;
- keeps each agent's response state separate;
- preserves tags in each inbox item so newly tagged agents can see direct
  invocation;
- lets each associated agent decide whether to publish an outbox reply.

### Workspace Management

- The backend creates worktrees for chat workspaces and agent edit targets.
- Workspace creation should use existing project/worktree conventions where
  possible.
- Pressing `w` associates a fuzzy-found workspace with the current chat.
- Pressing `c` on a workspace sidebar row creates a workspace-scoped chat under
  that workspace.
- Pressing `c` on the sidebar `Chats` section creates a general agent chat.
- Agent processes spawn in their own session directories.
- The daemon writes the selected workspace path into the agent prompt and
  `AGENTS.md` context instead of using the workspace as process cwd.
- When the focused workspace changes, the daemon reloads affected agents after
  updating their prompt/context.
- A chat can link to multiple workspaces, but only one workspace is in focus by
  default.
- Deleting a chat deletes its associated worktrees.

### Routing and Message Send

When a user sends a message, the backend must know:

- chat id;
- message text;
- newly selected agent tokens and their stable agent profile ids;
- all agent ids already associated with the chat;
- persistent focused agent ids for summary and interrupt behavior;
- selected workspace focus;
- whether each new agent association is reusable or shadow-owned.

The backend should persist the user message and association metadata before
sending work to the daemon. The daemon should emit events that can be replayed
into the chat transcript after reconnect.

When the user sends a message, the backend delivers it to every agent already
associated with the chat. Agent tokens in the message add new associations
before delivery, so newly tagged agents receive the message that tagged them.
Persistent focus does not limit delivery; it controls the focused agent shown in
the UI and the default interrupt target.

### Agent Outbox Contract

Each agent session gets an outbox and prompt instructions that describe how to
publish Controller-visible replies. The session `AGENTS.md` must tell the agent:

- the current chat id;
- the outbox location or command;
- the target workspace paths where it should make edits;
- whether it was newly tagged in the current message;
- the original message text with agent tags preserved;
- that inbox items may be acknowledged internally without publishing a
  transcript reply.

The Controller transcript consumes explicit outbox items. The daemon may still
capture runtime stdout for logs, debugging, and health checks, but raw stdout
does not become a chat reply by itself.

### Suggested API Surface

This is a product-level sketch, not final route naming.

```text
POST   /chats
GET    /chats
GET    /chats/:id
DELETE /chats/:id

POST   /chats/:id/messages
GET    /chats/:id/messages

GET    /agents?query=
POST   /chats/:id/agent-routes
DELETE /chats/:id/agent-routes/:route_id

GET    /workspaces?query=
GET    /chats/:id/workspaces
POST   /chats/:id/workspaces
PATCH  /chats/:id/workspaces/:workspace_id/focus
POST   /workspaces/:workspace_id/chats

GET    /daemon/sessions/:id/stream
POST   /daemon/sessions/:id/interrupt
```

### Daemon Events

The daemon event model from the RFC still applies. The product additionally
needs events or metadata for:

- user messages delivered to agent inboxes;
- agent association creation;
- agent inbox item creation;
- persistent agent focus changes;
- shadow agent session spawn;
- reusable session spawn on tag;
- shadow agent session cleanup;
- chat-level inbox fan-out;
- agent outbox item published;
- workspace association from fuzzy find;
- workspace-scoped chat creation;
- general chat creation from the sidebar `Chats` section;
- workspace focus changes;
- workspace path used for an inbox item.

## Lifecycle Rules

### Creating a Chat

1. User presses `n`.
2. Frontend creates a chat for the focused project.
3. Frontend focuses the composer.
4. Sidebar focuses the new chat row after the composer is blurred.

### Sending to `@agent`

1. User selects or types an `@agent` token.
2. Frontend stores the token as an agent association, not plain text only.
3. Frontend sets that agent as persistent focus for the chat.
4. Backend resolves the agent profile.
5. Daemon spawns or attaches to the reusable session at tagging time.
6. Send request includes association metadata.
7. Backend delivers the message to every associated agent inbox.
8. Chat transcript records the user message and published outbox replies.

### Sending to `%agent`

1. User selects or types a `%agent` token.
2. Frontend stores the token as a pending shadow-agent association.
3. No agent session is spawned yet.
4. Send request includes association metadata.
5. Backend resolves the agent profile.
6. Daemon spawns a new session with `owner_chat_id`.
7. Backend associates the new session with the chat.
8. Backend delivers the message to every associated agent inbox.
9. Chat transcript records the user message, the new shadow agent association,
   and published outbox replies.

### Sending to Associated Agents

1. User sends a message to the chat.
2. Backend resolves any newly tagged agents and adds them to the chat.
3. Backend loads all agents already associated with the chat.
4. Backend creates one independent inbox item per associated agent.
5. Each associated agent receives the message and workspace context.
6. Newly tagged agents receive the original tag in their inbox item.
7. Each associated agent decides whether to publish an outbox reply.
8. Transcript renders published outbox replies separately by agent.
9. Agents that do not publish an outbox reply do not appear in the transcript
   for that turn.

### Sending With Persistent Focus

1. User sends a message with no explicit agent token.
2. Backend delivers the message to every agent associated with the chat.
3. If no agents are associated with the chat, the UI should ask the user to tag
   or choose an agent rather than silently dropping the message.
4. Persistent focus still identifies the primary agent for display and
   interrupt behavior.

### Associating Workspace With `w`

1. User presses `w` from chat mode.
2. Frontend opens workspace fuzzy find.
3. User selects a workspace.
4. Backend records the `ChatWorkspaceLink` and marks focus if requested.
5. Backend spawns an agent worktree.
6. Backend updates the prompt and `AGENTS.md` context for associated agents.
7. Daemon reloads affected agents so future work targets the selected folder.

### Creating a Workspace-scoped Chat

1. User focuses a workspace row in the sidebar.
2. User presses `c`.
3. Backend creates a chat associated with that workspace.
4. Backend spawns an agent worktree.
5. Frontend nests the new chat under the workspace row and focuses the composer.

### Creating a General Agent Chat

1. User focuses the sidebar `Chats` section.
2. User presses `c`.
3. Backend creates a general agent chat without a workspace association.
4. Frontend places the new chat under the `Chats` section and focuses the
   composer.

### Deleting a Chat

1. Backend finds shadow agent sessions.
2. Backend asks the daemon to interrupt and despawn those sessions.
3. Backend deletes worktrees associated with the chat.
4. Backend permanently deletes the chat and its transcript records.
5. Reusable sessions linked through `@agent` remain running.

## Error States

- Agent profile no longer exists.
- Agent profile exists but its runtime is unavailable.
- Reusable agent failed to spawn.
- Shadow agent failed to spawn.
- Worktree creation failed.
- Worktree deletion failed during chat deletion.
- Workspace path no longer exists.
- Agent prompt or `AGENTS.md` reload failed after workspace focus changed.
- Daemon is unavailable.
- Daemon token is rejected.
- Message has no associated agent inbox target.
- Chat deletion cannot clean up a shadow agent session.

Each error should appear near the action that caused it. Errors that affect the
whole chat, such as daemon unavailability, can use the existing chat empty or
status states.

## Acceptance Criteria

### New Chat Flow

- Pressing `n` in chat mode creates a new chat for the focused project.
- The composer is focused after chat creation.
- Pressing `Esc` in the composer focuses the chat row in the sidebar.
- Pressing `Shift+Esc` interrupts a running turn and does not blur the composer
  as its primary behavior.

### Agent Routing

- Typing `@` opens reusable agent suggestions.
- Typing `%` opens shadow agent suggestions.
- Selecting either kind inserts a durable token.
- Selecting `@agent` sets persistent agent focus.
- Selecting `@agent` spawns or attaches to the reusable session at tagging time.
- Selecting `%agent` does not spawn a session until the message is sent.
- Sending a message with `@agent` associates a reusable session with the chat.
- Sending a message with `%agent` creates and associates a shadow agent session.
- Sending one message with multiple agent tokens associates all tagged agents.
- Every associated agent receives each sent chat message in its inbox.
- Newly tagged agents receive the message with their tag preserved.
- Each associated agent can choose whether to publish an outbox reply.
- Agents that do not publish an outbox reply do not produce transcript
  placeholders.
- Raw runtime stdout does not become a chat reply by itself.
- Deleting a chat despawns shadow agent sessions and leaves reusable sessions
  alone.

### Workspace Routing

- Pressing `j` and `k` moves through visible sidebar items.
- Pressing `l` on a highlighted chat row focuses the corresponding chat.
- Pressing `w` opens workspace fuzzy find.
- Selecting a workspace associates it with the chat and spawns an agent
  worktree.
- The chat can link to multiple workspaces.
- The summary pane shows all associated workspaces.
- Agent work uses the focused workspace path as prompt context.
- Agent sessions spawn in their own directories rather than inside worktrees.
- Workspace association or focus changes update agent prompt/context and reload affected
  agents.
- Pressing `c` on a workspace sidebar row creates a workspace-scoped chat under
  that workspace.
- Pressing `c` on the sidebar `Chats` section creates a general agent chat.
- Deleting a chat deletes its associated worktrees.

### Summary Pane

- The summary pane shows all agents associated with the chat.
- The summary pane shows all workspaces associated with the chat.
- The summary pane marks focused agents and workspaces when present.
- The summary pane updates after agent or workspace focus changes.
- Empty focus states do not block composing or sending.

### Durability

- Chat messages and agent association metadata survive browser reload.
- Daemon sessions remain replayable through the daemon event stream.
- Shadow agent session ownership survives reload so cleanup still works.
- Chat deletion permanently deletes the chat transcript records.

## Metrics

- Time from pressing `n` to focused composer.
- Number of new chats created without opening an agent modal.
- Frequency of `@agent` versus `%agent` usage.
- Failed agent spawn rate.
- Failed worktree creation rate.
- Shadow agent sessions left running after chat deletion.
- Worktrees left on disk after chat deletion.
- Message sends with invalid or stale tokens.
- Agent inbox delivery failures.
- Agent prompt/context reload failures.

## Decision Log

1. Selecting `@agent` sets persistent agent focus for future messages.
2. Selecting `%agent` creates a pending shadow association. The daemon spawns
   the shadow agent session only when the message is sent.
3. Once an agent has been tagged in a chat, every later message sent to that
   chat goes to that agent's inbox.
4. One message can tag multiple agents. Each associated agent gets an
   independent inbox item and reply state.
5. Newly tagged agents see the message with their tag preserved, so they can
   treat the message as a direct invocation.
6. Each associated agent may decide whether to respond.
7. Agents publish Controller-visible replies through an explicit outbox rather
   than relying on raw stdout.
8. Chat deletion is permanent for the chat and transcript records.
9. Deleting a chat deletes its associated worktrees.
10. Agents spawn in their own session directories. Workspace focus updates the
   agent prompt and `AGENTS.md` context, then reloads the agent.
11. The summary pane shows all agents and workspaces associated with the current
   chat, including idle ones.
12. Pressing `w` fuzzy-finds a workspace, associates it with the current chat,
    and spawns an agent worktree.
13. Pressing `c` on a workspace sidebar row creates a workspace-scoped chat
    under that workspace.
14. Pressing `c` on the sidebar `Chats` section creates a general agent chat.

## Proposed Phases

### Phase 1: Composer-first Chat

- Replace default new-chat modal with `n` creating an empty chat.
- Focus the composer immediately.
- Make `Esc` return focus to the sidebar row.
- Keep existing daemon-backed send and transcript behavior.

### Phase 2: Agent Token Suggestions

- Add `@` and `%` token detection.
- Add keyboard-navigable suggestions from configured agent profiles.
- Persist agent association metadata with sent messages.
- Render newly tagged and associated agents in the transcript.

### Phase 3: Agent Ownership Lifecycle

- Implement reusable session spawn on tag for `@agent`.
- Implement shadow agent session spawn for `%agent`.
- Implement chat-level inbox fan-out and optional outbox replies.
- Implement chat deletion cleanup for owned sessions.
- Add lifecycle tests around deletion and daemon reconnect.

### Phase 4: Workspace Sidebar and Focus

- Add `w` workspace fuzzy find.
- Create and link worktree-backed workspaces.
- Add workspace rows and a `Chats` section to the sidebar.
- Implement `c` for workspace-scoped chat creation.
- Implement `c` for general agent-chat creation from the `Chats` section.
- Add summary pane workspace focus.
- Pass focused workspace paths into agent prompt/context.
- Reload affected agents after workspace focus changes.
- Delete associated worktrees when deleting a chat.

### Phase 5: Summary and Polish

- Finalize summary pane states for all associated agents and workspaces.
- Improve invalid token and unavailable daemon errors.
- Add metrics and diagnostics for leaked owned sessions.
