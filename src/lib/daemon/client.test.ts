import { describe, it, expect, vi, beforeEach } from "vitest";
import { DaemonClient, DaemonHttpError } from "./client";

function mockFetch(responses: Array<{ status: number; body: unknown }>) {
  const queue = [...responses];
  return vi.fn(async () => {
    const r = queue.shift()!;
    return {
      ok: r.status >= 200 && r.status < 300,
      status: r.status,
      json: async () => r.body,
      text: async () => JSON.stringify(r.body),
    } as any;
  });
}

describe("DaemonClient", () => {
  beforeEach(() => vi.restoreAllMocks());

  it("calls same-origin /api/daemon routes without exposing auth", async () => {
    const fetchMock = mockFetch([{ status: 200, body: [] }]);
    vi.stubGlobal("fetch", fetchMock);

    const c = new DaemonClient("/api/daemon");
    await c.listSessions();

    const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe("/api/daemon/sessions");
    expect(init.headers).not.toMatchObject({ Authorization: expect.any(String) });
  });

  it("builds same-origin websocket URLs", () => {
    const c = new DaemonClient("/api/daemon");
    expect(c.wsUrl("s1", 5)).toBe("/api/daemon/sessions/s1/stream?since=5");
  });

  it("throws DaemonHttpError on 404", async () => {
    vi.stubGlobal("fetch", mockFetch([{ status: 404, body: { error: "not found" } }]));
    const c = new DaemonClient("/api/daemon");
    await expect(c.getSession("missing")).rejects.toMatchObject({
      name: "DaemonHttpError",
      status: 404,
    });
  });

  it("creates chats through same-origin daemon routes", async () => {
    const chat = { id: "chat-1", project_id: "project-1", title: "Plan", created_at: 1, updated_at: 1, deleted_at: null };
    const fetchMock = mockFetch([{ status: 200, body: chat }]);
    vi.stubGlobal("fetch", fetchMock);

    const c = new DaemonClient("/api/daemon");
    await expect(c.createChat({ project_id: "project-1", title: "Plan" })).resolves.toEqual(chat);

    const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe("/api/daemon/chats");
    expect(init.method).toBe("POST");
    expect(JSON.parse(init.body as string)).toEqual({ project_id: "project-1", title: "Plan" });
    expect(init.headers).not.toMatchObject({ Authorization: expect.any(String) });
  });

  it("lists agent profiles without auth headers", async () => {
    const profiles = [
      {
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
      },
    ];
    const fetchMock = mockFetch([{ status: 200, body: profiles }]);
    vi.stubGlobal("fetch", fetchMock);

    const c = new DaemonClient("/api/daemon");
    await expect(c.listProfiles()).resolves.toEqual(profiles);

    const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe("/api/daemon/profiles");
    expect(init.headers).not.toMatchObject({ Authorization: expect.any(String) });
  });

  it("sends chat messages with route tokens", async () => {
    const response = {
      message: {
        id: "message-1",
        chat_id: "chat-1",
        idempotency_id: "idem-1",
        body: "@planner hello",
        token_spans: [{ kind: "reusable", handle: "planner", start: 0, end: 8 }],
        created_at: 3,
      },
      turns: [
        {
          id: "turn-1",
          session_id: "session-1",
          chat_id: "chat-1",
          chat_message_id: "message-1",
          inbox_seq: 4,
          status: "queued",
          received_at: 5,
          activity_started_at: null,
          ended_at: null,
        },
      ],
    };
    const fetchMock = mockFetch([{ status: 202, body: response }]);
    vi.stubGlobal("fetch", fetchMock);

    const c = new DaemonClient("/api/daemon");
    await expect(
      c.sendChatMessage("chat-1", {
        idempotency_id: "idem-1",
        body: "@planner hello",
        tokens: [{ kind: "reusable", handle: "planner", start: 0, end: 8 }],
      }),
    ).resolves.toEqual(response);

    const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe("/api/daemon/chats/chat-1/messages");
    expect(init.method).toBe("POST");
    expect(JSON.parse(init.body as string)).toEqual({
      idempotency_id: "idem-1",
      body: "@planner hello",
      tokens: [{ kind: "reusable", handle: "planner", start: 0, end: 8 }],
    });
  });

  it("reads planned agent turn traces", async () => {
    const trace = [
      {
        turn: {
          id: "turn-1",
          session_id: "session-1",
          chat_id: "chat-1",
          chat_message_id: "message-1",
          inbox_seq: 4,
          status: "completed",
          received_at: 5,
          activity_started_at: 6,
          ended_at: 7,
        },
        metrics: {
          turn_id: "turn-1",
          input_tokens: 10,
          output_tokens: 20,
          cache_read_tokens: 0,
          cache_write_tokens: 0,
          tool_call_count: 1,
          outbox_write_count: 2,
          error_count: 0,
          updated_at: 8,
        },
        events: [],
      },
    ];
    const fetchMock = mockFetch([{ status: 200, body: trace }]);
    vi.stubGlobal("fetch", fetchMock);

    const c = new DaemonClient("/api/daemon");
    await expect(c.getAgentTrace("session-1")).resolves.toEqual(trace);

    const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe("/api/daemon/observability/agents/session-1");
    expect(init.headers).not.toMatchObject({ Authorization: expect.any(String) });
  });
});
