# PRD: Agent Observability

Date: 2026-04-29
Status: Draft

Related docs:

- `docs/plans/2026-04-29-controller-agent-product-prd.md`
- `docs/plans/2026-04-29-chat-routing-prd.md`
- `docs/plans/2026-04-27-controller-chat-modes-daemon-rfc.md`
- `docs/plans/2026-03-10-session-token-metrics-design.md`

## Table of Contents

- [Summary](#summary)
- [Design Assets](#design-assets)
- [Problem](#problem)
- [Goals](#goals)
- [Non-goals](#non-goals)
- [Users](#users)
- [Core Concepts](#core-concepts)
- [Product Principles](#product-principles)
- [Metrics Store](#metrics-store)
- [Views](#views)
- [Turn Model](#turn-model)
- [User Stories](#user-stories)
- [Product Requirements](#product-requirements)
- [Frontend UX Requirements](#frontend-ux-requirements)
- [Backend and Daemon Requirements](#backend-and-daemon-requirements)
- [Acceptance Criteria](#acceptance-criteria)
- [Open Questions](#open-questions)
- [Phases](#phases)

## Summary

Agent observability gives the user two levels of visibility:

1. Chat-level metrics for cost and performance.
2. Agent-level turn traces for thinking, tool use, output writes, and runtime
   state.

The Controller should record a shared observability store for agent sessions,
turns, events, and metrics. The UI should then project that store into two
surfaces:

- chat tabs with `Chat` and `Metrics`;
- an agent observability page with turn-first traces.

The chat transcript remains intentional. It shows user messages and agent
outbox replies. It does not show runtime stdout as product output, and it does
not get a Thinking tab.

## Design Assets

- [Agent Observability Mode mockup](../assets/design/controller-agent-status-ui.png)

## Problem

The Controller is moving from terminal orchestration toward agent orchestration.
Once agents run for minutes, span workspaces, call tools, and choose when to
publish outbox replies, users need answers to common questions:

- Did the agent receive my message?
- Is it working, waiting, blocked, or done?
- How long did the turn take?
- How many tokens did it burn?
- Which tools did it call?
- What runtime-exposed thinking or reasoning summary did Codex provide?
- Did it publish anything to the chat outbox?
- If nothing appeared in chat, did the agent choose silence or fail?

The current chat transcript cannot answer those questions by itself. It should
not try to become a runtime log. The user needs metrics in chat and detailed
agent traces on the agent page.

## Goals

1. Define a durable metrics and observability store for agent sessions.
2. Define chat-level metrics that summarize cost and performance.
3. Define an agent observability page organized by turns.
4. Preserve the outbox contract for chat replies.
5. Make long-running reusable agents and short-lived shadow agents inspectable.
6. Show runtime-exposed thinking, tools, output writes, tokens, timing, errors,
   and lifecycle state without scraping raw stdout as product state.
7. Center observability on per-agent turn traces instead of a merged
   multi-agent thinking pipeline.

## Non-goals

- A merged multi-agent thinking timeline.
- A Thinking tab inside the chat window.
- Showing hidden model chain-of-thought.
- Treating terminal stdout as chat output.
- A general-purpose analytics warehouse.
- Cross-device or multi-user observability.
- Provider-independent coverage for data the runtime does not expose.
- Final persistence schema design.

## Users

### Interactive Developer

Works in the Controller as a daily local command center. They want to know
whether an agent is making progress, stuck, spending too many tokens, or waiting
for input.

### Agent Orchestrator

Runs reusable specialists and short-lived shadow agents. They need to inspect a
single agent's turns without merging unrelated agents into one trace.

### Debugging Maintainer

Investigates runtime failures, tool-call failures, token spikes, and long
turns. They need durable facts they can inspect after the original chat view has
changed.

## Core Concepts

### Observability Store

A durable store of agent turns, events, metrics, lifecycle states, and links to
chats and workspaces.

### Agent Turn

One unit of work for one agent. A turn starts when the agent receives an inbox
item. A turn ends when the agent reaches a user-meaningful terminal state.

### Thinking Event

A runtime-exposed thinking, reasoning, planning, or progress item. The
Controller records only what Codex or another runtime exposes through its
observable interface. It does not infer or fabricate hidden reasoning.

### Outbox Write

An explicit agent action that publishes a Controller-visible reply or status
item. The chat transcript consumes outbox writes. An outbox write is an event
inside a turn, not the turn boundary.

### Chat Metrics

Aggregated cost and performance data for a chat: tokens, elapsed time, turn
counts, tool counts, slow turns, errors, and active agents.

### Agent Trace

The turn-first history for a single agent session. Each turn expands to show
the events and metrics collected for that unit of work.

## Product Principles

### Chat Shows What The Agent Says

The `Chat` tab shows user messages and agent outbox replies. Agents may receive
messages, think, call tools, and decide not to reply. The transcript should not
add placeholders for silence.

### Metrics Explain Cost

The `Metrics` tab tells the user what the chat cost in tokens and time. It
summarizes the work without turning the chat into a debug console.

### Agent Pages Explain Behavior

The agent observability page tells the user what one agent did, turn by turn.
It should answer "why is this agent in this state?" better than the chat can.

### Runtime Facts Beat Terminal Scraping

The Controller should use structured events from the daemon and runtime. Raw
stdout can remain available for low-level diagnostics, but it should not drive
chat output or primary observability state.

### Turn Boundaries Match User Expectations

Users think a turn starts when the agent receives work and ends when the agent
is ready for the next instruction, waiting on the user, interrupted, failed, or
abandoned.

## Metrics Store

The metrics store should record facts at three levels:

- chat;
- agent session;
- agent turn.

### Stored Records

The store should support records for:

- agent session lifecycle;
- chat and workspace links;
- inbox item receipt;
- turn state transitions;
- runtime-exposed thinking events;
- tool calls;
- tool results;
- tool approvals when applicable;
- outbox writes;
- token usage;
- timing;
- errors;
- interruptions;
- resumes;
- cancellations.

### Chat-Level Aggregates

For each chat, the store should support aggregate metrics:

- total token use;
- token use by agent;
- token use by turn;
- total elapsed agent time;
- elapsed time by agent;
- elapsed time by turn;
- count of turns;
- count of tool calls;
- count of failed or interrupted turns;
- slowest turns;
- most expensive turns.

### Agent-Level Aggregates

For each agent session, the store should support:

- total turns;
- active turn state;
- total token use;
- total active time;
- total wait or startup time;
- tool-call count;
- outbox-write count;
- error count;
- linked chats;
- linked workspaces;
- current runtime status.

### Runtime Data Availability

The Controller should store a field only when the runtime exposes it. Codex may
expose tool use, token counts, thinking output, status transitions, and timing
with different completeness than other runtimes. The UI should distinguish
missing data from zero values.

## Views

### Chat Tabs

The chat window has two tabs:

- `Chat`
- `Metrics`

The chat window does not have a `Thinking` tab.

### Chat Tab

The `Chat` tab renders:

- user messages;
- explicit agent outbox replies;
- agent identity on each outbox reply;
- quiet system state only when it affects the user's next action.

The `Chat` tab does not render:

- raw runtime stdout;
- thinking events;
- tool-call logs as the main conversation;
- placeholders for agents that received a message but did not reply.

### Metrics Tab

The `Metrics` tab renders chat-level aggregates:

- total tokens;
- tokens by agent;
- tokens by turn;
- total elapsed turn time;
- average turn time;
- slowest turns;
- tool-call count;
- error and interruption count;
- active agents and their current states.

Rows that name an agent should link to that agent's observability page.

### Agent Observability Page

The agent observability page shows one agent session. It should work for:

- reusable long-running agents;
- short-lived shadow agents;
- completed agents with retained observability history.

The page layout should prioritize:

- agent identity, runtime, status, profile version, and ownership;
- linked chats and workspaces;
- current or most recent turn;
- turn list;
- per-turn expandable details.

### Agent Turn List

The turn list is the primary agent trace. Each turn row should show:

- turn number or stable id;
- source inbox item;
- linked chat;
- status;
- received time;
- duration;
- token count;
- tool-call count;
- outbox-write count;
- error indicator when applicable.

Expanding a turn should show:

- inbox payload summary;
- thinking events;
- tool calls and results;
- outbox writes;
- token breakdown when exposed;
- timing breakdown;
- errors, interruptions, or approvals;
- terminal state.

## Turn Model

### Turn Start

A turn starts when an agent receives an inbox item.

This start point captures the latency a user feels. If the agent sits in a
queue, starts slowly, reloads context, or waits for runtime activity, the turn
still started from the user's point of view.

### Turn Timing

Each turn should track:

- `received_at`: the agent accepted the inbox item;
- `activity_started_at`: the first runtime activity event for the turn, if any;
- `ended_at`: the turn reached a terminal state.

The UI can derive:

- total turn time: `ended_at - received_at`;
- runtime active time: `ended_at - activity_started_at`;
- wait or startup time: `activity_started_at - received_at`.

When `activity_started_at` is missing, the UI should show that runtime activity
has not started or was not observed.

### Turn End

A turn ends when the agent reaches one terminal state:

- `completed`: the runtime reports completion and the agent is idle;
- `waiting_for_input`: the agent needs user input;
- `waiting_for_approval`: the agent needs a tool approval;
- `interrupted`: the user stopped the turn;
- `failed`: the runtime crashed, errored, or the daemon lost the turn;
- `abandoned`: a newer inbox item superseded an old pending turn.

An outbox write does not end a turn. An agent may write to the outbox and keep
working, or it may finish useful work without writing to the outbox.

## User Stories

- As a developer, I want to see total token use for a chat so I understand how
  expensive the conversation was.
- As a developer, I want to see time per turn so I can tell which requests were
  slow.
- As a developer, I want to open an agent page from chat metrics so I can debug
  one agent's behavior.
- As a developer, I want each agent turn to show thinking, tools, outbox writes,
  tokens, and timing so I can reconstruct what happened.
- As a developer, I want silent agents to stay out of the chat transcript so the
  conversation stays readable.
- As a developer, I want silent agents to remain inspectable on the agent page
  so I can tell whether silence was intentional.
- As a developer, I want missing runtime data to look different from zero usage
  so I do not misread incomplete instrumentation.
- As a developer, I want completed shadow agents to keep their observability
  history so I can debug them after the owning chat has moved on.

## Product Requirements

1. The Controller records agent observability data in a durable store.
2. The store links metrics to chats, agent sessions, turns, and workspaces.
3. A turn starts when an agent receives an inbox item.
4. A turn ends only when the agent reaches a terminal state.
5. Outbox writes are recorded as events inside turns.
6. The `Chat` tab renders user messages and outbox replies only.
7. The chat window exposes a `Metrics` tab.
8. The chat window does not expose a `Thinking` tab.
9. The `Metrics` tab aggregates token use, timing, tool counts, and errors for
   the chat.
10. Agent metric rows link to the agent observability page.
11. The agent observability page shows one agent session at a time.
12. The agent observability page is organized by turns first.
13. Each expanded turn shows thinking events, tool calls, outbox writes, token
    use, timing, and errors when available.
14. The UI distinguishes missing runtime data from zero values.
15. The product does not plan a merged multi-agent thinking pipeline.

## Frontend UX Requirements

### Chat Shell

- Place `Chat` and `Metrics` tabs above the chat window.
- Preserve the current chat reading experience as the default `Chat` tab.
- Keep the `Metrics` tab dense and scannable.
- Link agent names in metrics rows to their agent observability pages.
- Do not show a `Thinking` tab in chat.

### Metrics Tab

- Show chat totals first.
- Show per-agent metrics after chat totals.
- Show turn-level rows when the user needs detail.
- Mark active, waiting, failed, and interrupted turns.
- Use clear empty states when token or timing data is unavailable.
- Keep missing metrics visually distinct from zero values.

### Agent Observability Page

- Show the agent's status and ownership at the top.
- Show linked chats and workspaces near the top.
- Show the current turn before historical turns when the agent is active.
- List turns in reverse chronological order by default.
- Let users expand a turn without navigating away.
- Keep thinking, tools, outbox writes, and errors visually distinct inside an
  expanded turn.
- Let users jump from an outbox write back to the chat message that rendered it.

## Backend and Daemon Requirements

### Core Entities

The product needs durable records equivalent to:

```text
AgentSession
AgentTurn
AgentObservationEvent
AgentTurnMetric
ChatMetric
AgentChatLink
AgentWorkspaceLink
```

Final table names may differ. The product requirement is the relationship:
events and metrics must be replayable by chat, agent, and turn.

### Event Types

The daemon should emit or persist event types for:

- inbox received;
- turn started;
- runtime activity started;
- thinking event observed;
- tool call started;
- tool call completed;
- tool approval requested;
- tool approval resolved;
- outbox write published;
- token usage observed;
- turn completed;
- turn waiting for input;
- turn waiting for approval;
- turn interrupted;
- turn failed;
- turn abandoned;
- agent session started;
- agent session resumed;
- agent session ended.

### Runtime Integration

The daemon should prefer structured runtime output. Codex-exposed tool use,
thinking output, token usage, and status events should map into the observation
store.

When a runtime lacks a field, the daemon should omit that metric or mark it
unavailable. It should not invent token counts, thinking text, or terminal
states.

### Retention

The store should retain observability history for completed reusable agents and
completed shadow agents. The PRD does not set a retention limit. A later design
can define pruning, export, or compaction.

## Acceptance Criteria

### Store

- Sending a chat message to an associated agent creates one agent turn for that
  agent.
- The turn records `received_at`.
- Runtime activity records `activity_started_at` when observed.
- Terminal states record `ended_at`.
- Tool use, token usage, thinking events, outbox writes, and errors attach to
  the correct turn.
- Replaying stored records can reconstruct the chat metrics and agent turn
  page.

### Chat Views

- The chat window shows `Chat` and `Metrics` tabs.
- The chat window does not show a `Thinking` tab.
- The `Chat` tab renders explicit outbox replies and does not render raw stdout
  as replies.
- The `Metrics` tab shows chat totals and per-agent metrics.
- Agent rows in chat metrics link to agent observability pages.

### Agent Views

- Opening an agent observability page shows the selected agent's turns.
- Each turn row shows status, duration, token count when available, tool-call
  count, outbox-write count, and linked chat.
- Expanding a turn shows thinking events, tool calls, tool results, outbox
  writes, errors, and timing breakdown when available.
- Silent turns remain visible on the agent page even when they produced no chat
  transcript reply.

## Open Questions

1. Which Codex events expose token usage, thinking output, and terminal state
   with enough structure for v1?
2. Should completed shadow-agent observability persist forever, follow chat
   retention, or use a separate pruning policy?
3. Should the agent observability page live under the existing Agents mode, or
   should it get its own route from chat metric links?
4. Should the Metrics tab show turn-level rows by default or hide them behind a
   drill-down?
5. What should the UI call unavailable metrics: `unavailable`, `not reported`,
   or another compact label?

## Phases

### Phase 1: Metrics Store

- Add durable records for turns, events, and metrics.
- Capture inbox receipt, runtime activity start, terminal state, outbox writes,
  token usage, tool calls, thinking events, and errors when exposed.
- Support replay by chat id and agent session id.

### Phase 2: Chat Metrics

- Add `Chat` and `Metrics` tabs above the chat window.
- Keep chat transcript output tied to outbox replies.
- Render chat totals and per-agent metrics.
- Link metric rows to agent observability pages.

### Phase 3: Agent Observability Page

- Add turn-first agent pages.
- Show status, ownership, linked chats, linked workspaces, and turn history.
- Add expandable turn details for thinking, tools, outbox writes, tokens,
  timing, and errors.

### Phase 4: Retention And Polish

- Define retention behavior for completed shadow agents.
- Add filters for status, chat, workspace, and error state.
- Add export or copy affordances for debugging traces if needed.
