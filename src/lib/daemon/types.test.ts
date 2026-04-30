import { describe, it, expect, expectTypeOf } from "vitest";
import fixture from "./__fixtures__/events.json";
import type {
  AgentProfile,
  AgentProfileVersion,
  AgentTurnTrace,
  Chat,
  ChatMetrics,
  ChatMessage,
  EventRecord,
  OutboxEvent,
  RouteToken,
  TurnMetrics,
} from "./types";

describe("daemon event types", () => {
  it("parses fixture events without type errors", () => {
    const events = fixture as EventRecord[];
    expect(events.length).toBeGreaterThan(0);
    for (const e of events) {
      expect(["inbox", "outbox", "system"]).toContain(e.channel);
      expect(typeof e.seq).toBe("number");
    }
  });

  it("narrows outbox agent_text payload", () => {
    const e: OutboxEvent = {
      kind: "agent_text",
      payload: { message_id: "m1", block_id: "b1", text: "hello" },
    };
    expect(e.payload.text).toBe("hello");
  });

  it("models tagged chat route tokens and chat message spans", () => {
    const token: RouteToken = { kind: "shadow", handle: "critic", start: 0, end: 7 };
    const message: ChatMessage = {
      id: "message-1",
      chat_id: "chat-1",
      idempotency_id: null,
      body: "@critic check this",
      token_spans: [token],
      created_at: 10,
    };

    expect(message.token_spans).toEqual([{ kind: "shadow", handle: "critic", start: 0, end: 7 }]);
  });

  it("allows inbound chat messages to preserve non-route persisted token spans", () => {
    const persistedSpans = {
      source: "legacy-parser",
      spans: [{ byte_range: [0, 5], label: "mention", metadata: { agent_id: "profile-1" } }],
    };
    const message: ChatMessage = {
      id: "message-1",
      chat_id: "chat-1",
      idempotency_id: null,
      body: "hello",
      token_spans: persistedSpans,
      created_at: 10,
    };

    expect(message.token_spans).toEqual(persistedSpans);
  });

  it("models profile chat metrics and trace payloads additively", () => {
    const profile: AgentProfile = {
      id: "profile-1",
      handle: "planner",
      name: "Planner",
      description: "Plans work",
      runtime: "claude",
      skills: ["planning"],
      prompt: "Plan carefully",
      archived_at: null,
      avatar_asset_path: null,
      avatar_status: "none",
      avatar_error: null,
      active_version_id: "version-1",
      created_at: 1,
      updated_at: 2,
    };
    const chat: Chat = {
      id: "chat-1",
      project_id: "project-1",
      title: "Plan",
      created_at: 3,
      updated_at: 4,
      deleted_at: null,
    };
    const metrics: ChatMetrics = {
      chat_id: chat.id,
      turn_count: 1,
      input_tokens: 10,
      output_tokens: 20,
      cache_read_tokens: 0,
      cache_write_tokens: 0,
      tool_call_count: 2,
      outbox_write_count: 3,
      error_count: 0,
      updated_at: 5,
    };
    const trace: AgentTurnTrace = {
      turn: {
        id: "turn-1",
        session_id: "session-1",
        chat_id: chat.id,
        chat_message_id: "message-1",
        inbox_seq: 7,
        status: "completed",
        received_at: 8,
        activity_started_at: 9,
        ended_at: 10,
      },
      metrics: {
        turn_id: "turn-1",
        input_tokens: 10,
        output_tokens: 20,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        tool_call_count: 2,
        outbox_write_count: 3,
        error_count: 0,
        updated_at: 11,
      },
      events: [
        {
          session_id: "session-1",
          seq: 1,
          channel: "outbox",
          kind: "agent_text",
          payload: {},
          created_at: 12,
          applied_at: null,
          chat_id: chat.id,
          chat_seq: 1,
          turn_id: "turn-1",
        },
      ],
    };

    expect(profile.handle).toBe("planner");
    expect(metrics.chat_id).toBe("chat-1");
    expect(trace.events[0].turn_id).toBe("turn-1");
  });

  it("models daemon profile strings as non-null and saved versions as non-null instructions", () => {
    expectTypeOf<AgentProfile["description"]>().toEqualTypeOf<string>();
    expectTypeOf<AgentProfile["avatar_status"]>().toEqualTypeOf<string>();
    expectTypeOf<AgentProfileVersion["default_workspace_behavior"]>().toEqualTypeOf<string>();
    expectTypeOf<AgentProfileVersion["outbox_instructions"]>().toEqualTypeOf<string>();
  });

  it("models nullable turn token metrics and traces without metrics", () => {
    const metrics: TurnMetrics = {
      turn_id: "turn-1",
      input_tokens: null,
      output_tokens: null,
      cache_read_tokens: null,
      cache_write_tokens: null,
      tool_call_count: 0,
      outbox_write_count: 0,
      error_count: 0,
      updated_at: 11,
    };
    const trace: AgentTurnTrace = {
      turn: {
        id: "turn-1",
        session_id: "session-1",
        chat_id: "chat-1",
        chat_message_id: "message-1",
        inbox_seq: 7,
        status: "completed",
        received_at: 8,
        activity_started_at: 9,
        ended_at: 10,
      },
      metrics: null,
      events: [],
    };

    expect(metrics.input_tokens).toBeNull();
    expect(trace.metrics).toBeNull();
  });
});
