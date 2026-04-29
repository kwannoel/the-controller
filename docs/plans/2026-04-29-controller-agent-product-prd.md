# PRD: Controller Agent Product Surface

Date: 2026-04-29
Status: Draft

Related docs:

- `docs/plans/2026-04-29-chat-routing-prd.md`
- `docs/plans/2026-04-29-agent-creation-prd.md`
- `docs/plans/2026-04-29-agent-observability-prd.md`
- `docs/plans/2026-04-27-controller-chat-modes-daemon-rfc.md`
- `docs/plans/2026-04-22-chat-mode-design.md`
- `docs/keyboard-modes.md`

## Table of Contents

- [Summary](#summary)
- [Design Assets](#design-assets)
- [Problem](#problem)
- [Goals](#goals)
- [Modes](#modes)
- [Mode Relationships](#mode-relationships)
- [Core Concepts](#core-concepts)
- [Use Cases](#use-cases)
- [User Stories](#user-stories)
- [Product Requirements](#product-requirements)
- [Non-goals](#non-goals)
- [Backend and Daemon Requirements](#backend-and-daemon-requirements)
- [Acceptance Criteria](#acceptance-criteria)
- [Open Questions](#open-questions)
- [Phases](#phases)

## Summary

The Controller should organize agent work around four product modes:

1. Chat Routing Mode
2. Agent Observability Mode
3. Agent Creation Mode
4. Workflow Creation Mode

Chat Routing Mode is specified in
`docs/plans/2026-04-29-chat-routing-prd.md`. This PRD defines the higher-level
product surface that surrounds it.

The product goal is to let a developer create agents, inspect their behavior,
compose workflows, and direct work from chat without losing keyboard flow or
local process control.

## Problem

The Controller is moving from terminal-session orchestration toward agent
orchestration. Chat routing explains how a user talks to agents, but it does
not cover the full lifecycle:

- Users need to see what agents are doing across sessions.
- Users need to create and edit agent definitions.
- Users need to compose repeatable workflows from agent capabilities.
- Users need chat, agents, workflows, and workspaces to share one model instead
  of becoming separate tools.

## Goals

1. Define the top-level modes for agent work in the Controller.
2. Link the detailed Chat Routing PRD as the chat-mode source of truth.
3. Specify product responsibilities for observing agents.
4. Specify product responsibilities for creating agents.
5. Specify product responsibilities for creating workflows.
6. Keep all modes compatible with daemon-owned agent sessions and event logs.
7. Keep the UI keyboard-first, dense, and inspectable.

## Modes

### 1. Chat Routing Mode

Source PRD: `docs/plans/2026-04-29-chat-routing-prd.md`

Chat Routing Mode is the operational surface for directing work. It lets users
create chats, associate workspaces, tag reusable agents with `@agent`, spawn
shadow agents with `%agent`, and send each chat message to associated agent
inboxes.

Product responsibilities:

- create and focus chats;
- route chat messages to associated agent inboxes;
- render only published outbox replies in the transcript;
- associate workspace worktrees with chats;
- show all chat-associated agents and workspaces in the summary pane;
- support sidebar navigation and chat creation from workspace rows or the
  `Chats` section.

### 2. Agent Observability Mode

Agent Observability Mode shows what agents are doing and why. It should answer
questions such as:

- Which agents exist?
- Which agents are running, idle, blocked, or failed?
- Which chats and workspaces is an agent associated with?
- What inbox messages has the agent received?
- What outbox replies has the agent published?
- What tools, approvals, errors, reloads, and workspace updates happened?

Product responsibilities:

- show a global agent list with status, runtime, profile, ownership, and recent
  activity;
- show an agent detail view with inbox, outbox, tool calls, errors, reloads,
  prompt/context version, and linked chats/workspaces;
- distinguish reusable agents from shadow agents;
- expose event replay and state transitions without showing raw runtime output
  as product truth;
- make silent agents legible through inbox state without adding transcript
  placeholders.

Primary user outcome: a developer can explain an agent's current state without
digging through terminal logs.

### 3. Agent Creation Mode

Source PRD: `docs/plans/2026-04-29-agent-creation-prd.md`

Agent Creation Mode lets users define agent profiles that can later be used in
chat, workflows, or observability.

Product responsibilities:

- create, edit, duplicate, archive, and test agent profiles;
- define name, handle, runtime, model/provider where applicable, prompt,
  skills, default workspace behavior, and outbox instructions;
- validate handles used by `@agent` and `%agent`;
- preview the generated prompt and `AGENTS.md` context before launch;
- version profile changes so existing sessions remain explainable;
- start a smoke-test chat or dry run from the profile editor.

Primary user outcome: a developer can create a reusable specialist without
hand-editing daemon folders or prompt files.

### 4. Workflow Creation Mode

Workflow Creation Mode lets users compose repeatable multi-agent procedures.
A workflow can define agent roles, workspace requirements, routing rules,
approval points, and completion conditions.

Product responsibilities:

- create, edit, duplicate, archive, and run workflows;
- define workflow inputs, required workspaces, agent profiles, shadow-agent
  steps, reusable-agent steps, and handoff rules;
- support sequential and parallel steps;
- define whether a step sends a message, waits for outbox, waits for tool
  approval, or checks a repository condition;
- show a workflow preview before launch;
- start a chat from a workflow run so the user can intervene.

Primary user outcome: a developer can turn a repeated agent collaboration into
a named workflow and run it without rebuilding the setup each time.

## Mode Relationships

```text
Agent Creation Mode
  -> creates AgentProfiles

Workflow Creation Mode
  -> references AgentProfiles and workspace requirements
  -> can launch chats or workflow runs

Chat Routing Mode
  -> associates AgentSessions and Workspaces with chats
  -> sends messages to agent inboxes
  -> renders published outbox replies

Agent Observability Mode
  -> reads AgentSessions, inboxes, outboxes, tools, errors, reloads, and links
```

Chat is the action surface. Observability is the inspection surface. Agent
Creation is the definition surface. Workflow Creation is the repeatability
surface.

## Core Concepts

### Agent Profile

A reusable definition for an agent. It includes handle, runtime, prompt, skills,
default context, and outbox instructions.

### Agent Session

A daemon-owned running or resumable process. Sessions may be reusable or
shadow-owned by a chat.

### Inbox Item

A durable message delivered to an agent. Chat messages fan out to every agent
associated with that chat.

### Outbox Item

A durable response the agent chooses to publish to the Controller. Chat
transcripts render outbox items. They do not render raw stdout or non-reply
placeholders.

### Workflow

A named recipe for coordinating agents, workspaces, messages, approvals, and
completion checks.

### Workspace

A project checkout or worktree that agents can target for edits. Agents spawn
in their own session directories; workspace paths enter agent prompt/context.

## Use Cases

### Create a Specialist

The user opens Agent Creation Mode, creates `reviewer`, configures its prompt
and skills, previews the generated context, and saves it. The agent becomes
available as `@reviewer` in chat and as a role in workflows.

### Watch a Running Agent

The user opens Agent Observability Mode, selects `%debugger`, and sees recent
inbox messages, published outbox replies, workspace context, tool calls, and
errors.

### Build a Repeatable Review Workflow

The user opens Workflow Creation Mode and defines a workflow with planner,
reviewer, and implementer roles. The workflow requires a workspace, runs the
planner first, fans work out to reviewer and implementer, and opens a chat for
human intervention.

### Route Ad Hoc Work From Chat

The user opens Chat Routing Mode, tags `@reviewer` and `%debugger`, associates a
workspace with `w`, and sends messages. The chat transcript shows only user
messages and published outbox replies.

## User Stories

- As a developer, I want to create agent profiles so I can reuse specialists
  across chats and workflows.
- As a developer, I want to inspect an agent's inbox and outbox so I can tell
  whether it saw a message and whether it chose to reply.
- As a developer, I want to see tool calls, errors, reloads, and workspace
  context for an agent so I can debug stuck or surprising behavior.
- As a developer, I want to compose workflows from agent profiles so repeated
  collaboration patterns become one command.
- As a developer, I want to launch or intervene in workflow runs through chat so
  automation stays steerable.
- As a developer, I want all modes to share handles, sessions, workspaces, and
  event history so the product feels like one system.

## Product Requirements

1. The product exposes Chat Routing, Agent Observability, Agent Creation, and
   Workflow Creation as distinct modes.
2. The mode switcher must make these modes discoverable without making chat
   slower.
3. Agent profiles created in Agent Creation Mode must appear in chat token
   suggestions and workflow builders.
4. Agent sessions created by chat or workflows must appear in Agent
   Observability Mode.
5. Workflows must reference agent profiles by stable id, not display name.
6. Observability must use durable daemon events as the source of truth.
7. Chat transcripts must render outbox items, not raw runtime stdout.
8. Workflow runs must produce observable events and linked chats.
9. Workspace associations must remain visible across chat, observability, and
   workflow surfaces.

## Non-goals

- Remote multi-user collaboration.
- A visual no-code automation canvas in v1.
- Arbitrary third-party runtime plugins.
- Showing every daemon diagnostic in the main chat transcript.
- Replacing GitHub issue and Kanban views in this PRD.
- Full implementation planning for each mode. Each mode should get a detailed
  design or implementation plan before build.

## Backend and Daemon Requirements

The daemon and backend need shared durable records for:

- agent profiles and profile versions;
- agent sessions;
- agent inbox items;
- agent outbox items;
- session ownership and chat links;
- workspace links and focused workspace;
- workflow definitions;
- workflow runs and step events;
- tool calls, approvals, errors, reloads, and status transitions.

The Controller backend should expose browser-safe endpoints for these records
and keep daemon authentication out of browser JavaScript.

## Acceptance Criteria

### Umbrella PRD

- The higher-level PRD links to `docs/plans/2026-04-29-chat-routing-prd.md`.
- The PRD names all four modes.
- Each requested mode has responsibilities, user outcomes, and acceptance
  criteria.

### Agent Observability Mode

- Users can list agents and see status, ownership, runtime, linked chats, and
  linked workspaces.
- Users can open an agent detail view with inbox, outbox, tool calls, errors,
  reloads, and prompt/context version.
- Agents that receive inbox messages but do not publish outbox replies do not
  create chat transcript placeholders.

### Agent Creation Mode

- Users can create and edit an agent profile.
- Users can configure handle, runtime, prompt, skills, workspace behavior, and
  outbox instructions.
- Saved profiles appear in chat suggestions and workflow builders.
- Existing sessions remain tied to the profile version that launched them.

### Workflow Creation Mode

- Users can create and edit a workflow definition.
- Users can add agent roles, workspace requirements, messages, approvals, and
  completion checks.
- Users can preview a workflow before running it.
- Workflow runs produce observable events and can open or link to a chat.

## Open Questions

1. Should Workflow Creation Mode start as a structured form, command palette, or
   graph-like builder?
2. Should Agent Observability Mode live as a full workspace mode or a pane that
   can be opened from chat?
3. Which workflow step types belong in v1: message, wait-for-outbox,
   tool-approval, repository-check, or manual checkpoint?
4. How should profile version diffs appear when an agent reloads after a prompt
   or workspace-context change?

## Phases

### Phase 1: Chat Routing

- Use `docs/plans/2026-04-29-chat-routing-prd.md` as the detailed source.
- Implement chat inbox fan-out, outbox-only transcript rendering, workspace
  association, shadow agents, and sidebar chat creation.

### Phase 2: Agent Creation

- Add agent profile CRUD.
- Add prompt and `AGENTS.md` preview.
- Add profile validation and smoke-test launch.
- Feed saved profiles into chat suggestions.

### Phase 3: Agent Observability

- Add agent list and detail views.
- Show inbox, outbox, tools, errors, reloads, workspace context, and linked
  chats.
- Add event replay filters and status transitions.

### Phase 4: Workflow Creation

- Add workflow definition CRUD.
- Add step authoring for agents, workspaces, messages, approvals, and checks.
- Add workflow preview and workflow run launch.
- Link workflow runs to chat and observability.
