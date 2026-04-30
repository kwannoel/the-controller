import { describe, expect, it } from "vitest";
import { render } from "@testing-library/svelte";
import AgentTraceView from "./AgentTraceView.svelte";
import type { AgentTurnTrace, Chat, ChatWorkspaceLink, DaemonSession } from "../daemon/types";

const session: DaemonSession = {
  id: "session-1",
  label: "Reviewer",
  agent: "codex",
  cwd: "/repo/controller",
  args: [],
  status: "running",
  native_session_id: "native-1",
  pid: 123,
  created_at: 1,
  updated_at: 2,
  ended_at: null,
  end_reason: null,
};

const chats = new Map<string, Chat>([
  ["chat-1", {
    id: "chat-1",
    project_id: "project-1",
    title: "Routing review",
    created_at: 1,
    updated_at: 2,
    deleted_at: null,
  }],
]);

const workspaceLinks: ChatWorkspaceLink[] = [
  {
    id: "workspace-1",
    chat_id: "chat-1",
    project_id: "project-1",
    workspace_id: "workspace-1",
    path: "/repo/controller",
    label: "controller",
    branch: "codex/routing",
    focused: true,
    created_at: 1,
    updated_at: 1,
  },
];

const traces: AgentTurnTrace[] = [
  {
    turn: {
      id: "turn-old",
      session_id: "session-1",
      chat_id: "chat-1",
      chat_message_id: "message-1",
      inbox_seq: 1,
      status: "completed",
      received_at: 1_000,
      activity_started_at: 1_100,
      ended_at: 1_900,
    },
    metrics: null,
    events: [
      {
        session_id: "session-1",
        seq: 1,
        channel: "inbox",
        kind: "user_text",
        payload: { text: "please review" },
        created_at: 1_000,
        applied_at: 1_000,
        chat_id: "chat-1",
        chat_seq: 1,
        turn_id: "turn-old",
      },
    ],
  },
  {
    turn: {
      id: "turn-new",
      session_id: "session-1",
      chat_id: "chat-1",
      chat_message_id: "message-2",
      inbox_seq: 2,
      status: "active",
      received_at: 2_000,
      activity_started_at: 2_050,
      ended_at: null,
    },
    metrics: {
      turn_id: "turn-new",
      input_tokens: 0,
      output_tokens: 14,
      cache_read_tokens: 0,
      cache_write_tokens: 0,
      tool_call_count: 1,
      outbox_write_count: 1,
      error_count: 0,
      updated_at: 2_100,
    },
    events: [
      {
        session_id: "session-1",
        seq: 2,
        channel: "outbox",
        kind: "agent_thinking",
        payload: { text: "checking the changed files" },
        created_at: 2_050,
        applied_at: 2_050,
        chat_id: "chat-1",
        chat_seq: 2,
        turn_id: "turn-new",
      },
      {
        session_id: "session-1",
        seq: 3,
        channel: "outbox",
        kind: "tool_call",
        payload: { tool: "shell", input: { cmd: "git diff" } },
        created_at: 2_060,
        applied_at: 2_060,
        chat_id: "chat-1",
        chat_seq: 3,
        turn_id: "turn-new",
      },
      {
        session_id: "session-1",
        seq: 4,
        channel: "outbox",
        kind: "agent_text",
        payload: { text: "No findings." },
        created_at: 2_100,
        applied_at: 2_100,
        chat_id: "chat-1",
        chat_seq: 4,
        turn_id: "turn-new",
      },
    ],
  },
];

describe("AgentTraceView", () => {
  it("renders the newest turn first with metrics and expandable runtime details", () => {
    const { getAllByText, getByText, getAllByTestId } = render(AgentTraceView, {
      session,
      traces,
      chats,
      workspaceLinks,
    });

    expect(getByText("Reviewer")).toBeTruthy();
    expect(getByText("codex")).toBeTruthy();
    expect(getByText("running")).toBeTruthy();
    expect(getAllByText("Routing review").length).toBeGreaterThan(0);
    expect(getByText("controller")).toBeTruthy();

    const rows = getAllByTestId("turn-row");
    expect(rows[0].textContent).toContain("turn-new");
    expect(rows[1].textContent).toContain("turn-old");
    expect(rows[0].textContent).toContain("14 tokens");
    expect(rows[0].textContent).toContain("1 tool");
    expect(rows[1].textContent).toContain("unavailable");

    expect(getByText("checking the changed files")).toBeTruthy();
    expect(getAllByText(/shell/).length).toBeGreaterThan(0);
    expect(getByText("No findings.")).toBeTruthy();
  });
});
