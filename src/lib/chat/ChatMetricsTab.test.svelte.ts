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
  total_elapsed_ms: 800,
  average_turn_ms: 800,
  slowest_turn_ms: 800,
  updated_at: 10,
  agents: [
    {
      session_id: "session-1",
      profile_id: "profile-1",
      profile_version_id: "version-1",
      route_type: "reusable",
      focused: true,
      token_source: "@reviewer",
      status: "running",
      turn_count: 1,
      input_tokens: 0,
      output_tokens: 20,
      cache_read_tokens: 0,
      cache_write_tokens: 0,
      total_tokens: 20,
      tool_call_count: 1,
      outbox_write_count: 1,
      error_count: 0,
      total_elapsed_ms: 800,
      average_turn_ms: 800,
      slowest_turn_ms: 800,
      current_turn_id: "turn-1",
      updated_at: 4,
    },
    {
      session_id: "session-2",
      profile_id: "profile-2",
      profile_version_id: "version-2",
      route_type: "shadow",
      focused: false,
      token_source: "%debugger",
      status: "running",
      turn_count: 1,
      input_tokens: null,
      output_tokens: null,
      cache_read_tokens: null,
      cache_write_tokens: null,
      total_tokens: null,
      tool_call_count: null,
      outbox_write_count: null,
      error_count: null,
      total_elapsed_ms: null,
      average_turn_ms: null,
      slowest_turn_ms: null,
      current_turn_id: "turn-2",
      updated_at: 5,
    },
  ],
  turns: [
    {
      turn_id: "turn-2",
      session_id: "session-2",
      chat_id: "chat-1",
      chat_message_id: "message-1",
      status: "active",
      received_at: 5,
      activity_started_at: null,
      ended_at: null,
      activity_latency_ms: null,
      duration_ms: null,
      input_tokens: null,
      output_tokens: null,
      cache_read_tokens: null,
      cache_write_tokens: null,
      total_tokens: null,
      tool_call_count: null,
      outbox_write_count: null,
      error_count: null,
      updated_at: 5,
    },
    {
      turn_id: "turn-1",
      session_id: "session-1",
      chat_id: "chat-1",
      chat_message_id: "message-1",
      status: "completed",
      received_at: 1,
      activity_started_at: 2,
      ended_at: 802,
      activity_latency_ms: 1,
      duration_ms: 801,
      input_tokens: 0,
      output_tokens: 20,
      cache_read_tokens: 0,
      cache_write_tokens: 0,
      total_tokens: 20,
      tool_call_count: 1,
      outbox_write_count: 1,
      error_count: 0,
      updated_at: 4,
    },
  ],
  workspace_links: [],
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
    expect(getByText("Elapsed")).toBeTruthy();
    expect(getAllByText("800ms").length).toBeGreaterThan(0);
    expect(getByText("Slowest")).toBeTruthy();
    expect(getAllByText("Reviewer").length).toBeGreaterThan(0);
    expect(getByText("@reviewer")).toBeTruthy();
    expect(getAllByText("running").length).toBeGreaterThan(0);
    expect(getAllByText("Debugger").length).toBeGreaterThan(0);
    expect(getByText("%debugger")).toBeTruthy();
    expect(getAllByText("unavailable").length).toBeGreaterThan(0);
    expect(getByText("Turn Metrics")).toBeTruthy();
    expect(getByText("turn-2")).toBeTruthy();
    expect(getByText("active")).toBeTruthy();
    expect(getByText("completed")).toBeTruthy();
    expect(getAllByText("1 outbox write").length).toBeGreaterThan(0);
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

  it("does not show stale fallback trace rows from another chat", () => {
    const emptyMetrics: ChatMetrics = {
      ...metrics,
      chat_id: "chat-empty",
      turn_count: 0,
      input_tokens: 0,
      output_tokens: 0,
      cache_read_tokens: 0,
      cache_write_tokens: 0,
      tool_call_count: 0,
      outbox_write_count: 0,
      error_count: 0,
      total_elapsed_ms: null,
      average_turn_ms: null,
      slowest_turn_ms: null,
      agents: [],
      turns: [],
    };
    const { getByText, queryByText } = render(ChatMetricsTab, {
      metrics: emptyMetrics,
      agentLinks: [],
      sessions,
      profiles: new Map(),
      traces,
      onOpenAgent: vi.fn(),
    });

    expect(getByText("No turns recorded for this chat.")).toBeTruthy();
    expect(queryByText("turn-1")).toBeNull();
    expect(queryByText("turn-2")).toBeNull();
  });
});
