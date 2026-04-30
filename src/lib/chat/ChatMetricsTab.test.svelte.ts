import { describe, expect, it, vi } from "vitest";
import { render, fireEvent } from "@testing-library/svelte";
import ChatMetricsTab from "./ChatMetricsTab.svelte";
import type { AgentTurnTrace, ChatAgentLink, ChatMetrics, DaemonSession } from "../daemon/types";

const metrics: ChatMetrics = {
  chat_id: "chat-1",
  turn_count: 2,
  input_tokens: 0,
  output_tokens: 20,
  cache_read_tokens: 0,
  cache_write_tokens: 0,
  tool_call_count: 1,
  outbox_write_count: 1,
  error_count: 0,
  updated_at: 10,
};

const agentLinks: ChatAgentLink[] = [
  {
    id: "link-1",
    chat_id: "chat-1",
    session_id: "session-1",
    profile_id: "profile-1",
    profile_version_id: "version-1",
    route_type: "reusable",
    focused: true,
    token_source: "@reviewer",
    created_at: 1,
  },
  {
    id: "link-2",
    chat_id: "chat-1",
    session_id: "session-2",
    profile_id: "profile-2",
    profile_version_id: "version-2",
    route_type: "shadow",
    focused: false,
    token_source: "%debugger",
    created_at: 2,
  },
];

const sessions = new Map<string, DaemonSession>([
  ["session-1", {
    id: "session-1",
    label: "Reviewer",
    agent: "codex",
    cwd: "/repo",
    args: [],
    status: "running",
    native_session_id: null,
    pid: 100,
    created_at: 1,
    updated_at: 2,
    ended_at: null,
    end_reason: null,
  }],
]);

const traces = new Map<string, AgentTurnTrace[]>([
  ["session-1", [
    {
      turn: {
        id: "turn-1",
        session_id: "session-1",
        chat_id: "chat-1",
        chat_message_id: "message-1",
        inbox_seq: 1,
        status: "completed",
        received_at: 1,
        activity_started_at: 2,
        ended_at: 3,
      },
      metrics: {
        turn_id: "turn-1",
        input_tokens: 0,
        output_tokens: 20,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        tool_call_count: 1,
        outbox_write_count: 1,
        error_count: 0,
        updated_at: 4,
      },
      events: [],
    },
  ]],
  ["session-2", [
    {
      turn: {
        id: "turn-2",
        session_id: "session-2",
        chat_id: "chat-1",
        chat_message_id: "message-1",
        inbox_seq: 2,
        status: "received",
        received_at: 5,
        activity_started_at: null,
        ended_at: null,
      },
      metrics: null,
      events: [],
    },
  ]],
]);

describe("ChatMetricsTab", () => {
  it("shows chat totals and keeps unavailable per-agent metrics distinct from zero", () => {
    const { getAllByText, getByText } = render(ChatMetricsTab, {
      metrics,
      agentLinks,
      sessions,
      profiles: new Map([
        ["profile-1", { id: "profile-1", handle: "reviewer", name: "Reviewer" }],
        ["profile-2", { id: "profile-2", handle: "debugger", name: "Debugger" }],
      ]),
      traces,
      onOpenAgent: vi.fn(),
    });

    expect(getAllByText("20 tokens").length).toBeGreaterThan(0);
    expect(getByText("2 turns")).toBeTruthy();
    expect(getAllByText("1 tool").length).toBeGreaterThan(0);
    expect(getByText("Reviewer")).toBeTruthy();
    expect(getByText("@reviewer")).toBeTruthy();
    expect(getByText("running")).toBeTruthy();
    expect(getByText("Debugger")).toBeTruthy();
    expect(getByText("%debugger")).toBeTruthy();
    expect(getAllByText("unavailable").length).toBeGreaterThan(0);
  });

  it("opens the agent observability workspace from an agent row", async () => {
    const onOpenAgent = vi.fn();
    const { getByRole } = render(ChatMetricsTab, {
      metrics,
      agentLinks,
      sessions,
      profiles: new Map([["profile-1", { id: "profile-1", handle: "reviewer", name: "Reviewer" }]]),
      traces,
      onOpenAgent,
    });

    await fireEvent.click(getByRole("button", { name: /open reviewer trace/i }));

    expect(onOpenAgent).toHaveBeenCalledWith("session-1");
  });
});
