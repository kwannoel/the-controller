import { describe, it, expect, vi, beforeEach } from "vitest";

describe("daemon store bootstrap", () => {
  beforeEach(() => vi.resetModules());

  it("sets reachable=true on successful same-origin ping without reading a token", async () => {
    const commandMock = vi.fn(async (cmd: string) => {
      throw new Error("unexpected command " + cmd);
    });
    vi.doMock("$lib/backend", () => ({
      command: commandMock,
      listen: () => () => {},
    }));
    const fetchMock = vi.fn(async () => ({ ok: true, status: 200, json: async () => [] } as any));
    vi.stubGlobal("fetch", fetchMock);
    const { daemonStore, bootstrap } = await import("./store.svelte");
    await bootstrap();
    expect(daemonStore.reachable).toBe(true);
    expect(commandMock).not.toHaveBeenCalled();
    expect("token" in daemonStore).toBe(false);
    const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe("/api/daemon/sessions");
    expect(init.headers).not.toMatchObject({ Authorization: expect.any(String) });
  });

  it("sets reachable=false when ping fails", async () => {
    const commandMock = vi.fn(async (cmd: string) => {
      throw new Error("unexpected command " + cmd);
    });
    vi.doMock("$lib/backend", () => ({
      command: commandMock,
      listen: () => () => {},
    }));
    vi.stubGlobal("fetch", vi.fn(async () => { throw new TypeError("connect refused"); }));
    const { daemonStore, bootstrap } = await import("./store.svelte");
    await bootstrap();
    expect(daemonStore.reachable).toBe(false);
    expect(commandMock).not.toHaveBeenCalled();
  });

  it("has separate reactive maps for profiles chats transcripts summaries and traces", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => ({ ok: true, status: 200, json: async () => [] } as any)));
    const { daemonStore } = await import("./store.svelte");

    expect(daemonStore.profiles.size).toBe(0);
    expect(daemonStore.chats.size).toBe(0);
    expect(daemonStore.activeChatId).toBeNull();
    expect(daemonStore.chatTranscripts.size).toBe(0);
    expect(daemonStore.chatSummaries.size).toBe(0);
    expect(daemonStore.agentTraces.size).toBe(0);
    expect("token" in daemonStore).toBe(false);
  });

  it("loads profiles chats transcripts and traces through the daemon client", async () => {
    const profile = {
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
    const chat = {
      id: "chat-1",
      project_id: "project-1",
      title: "Plan",
      created_at: 3,
      updated_at: 4,
      deleted_at: null,
    };
    const transcript = [
      {
        type: "user_message",
        message: {
          id: "message-1",
          chat_id: "chat-1",
          idempotency_id: null,
          body: "hello",
          token_spans: [],
          created_at: 5,
        },
      },
    ];
    const trace = [
      {
        turn: {
          id: "turn-1",
          session_id: "session-1",
          chat_id: "chat-1",
          chat_message_id: "message-1",
          inbox_seq: 6,
          status: "completed",
          received_at: 7,
          activity_started_at: 8,
          ended_at: 9,
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
          updated_at: 11,
        },
        events: [],
      },
    ];
    const metrics = {
      chat_id: "chat-1",
      turn_count: 1,
      input_tokens: 10,
      output_tokens: 20,
      cache_read_tokens: 0,
      cache_write_tokens: 0,
      tool_call_count: 1,
      outbox_write_count: 2,
      error_count: 0,
      updated_at: 12,
    };
    vi.stubGlobal("fetch", vi.fn());
    const {
      daemonStore,
      loadProfiles,
      loadChats,
      loadChatTranscript,
      loadChatLinks,
      loadAgentTrace,
      loadChatMetrics,
    } = await import("./store.svelte");
    daemonStore.client = {
      listProfiles: vi.fn(async () => [profile]),
      listChats: vi.fn(async () => [chat]),
      readChatTranscript: vi.fn(async () => transcript),
      listChatAgentLinks: vi.fn(async () => []),
      listChatWorkspaceLinks: vi.fn(async () => []),
      getAgentTrace: vi.fn(async () => trace),
      getChatMetrics: vi.fn(async () => metrics),
    } as any;

    await loadProfiles();
    await loadChats();
    await loadChatTranscript("chat-1");
    await loadChatLinks("chat-1");
    await loadAgentTrace("session-1");
    await loadChatMetrics("chat-1");

    expect(daemonStore.profiles.get("profile-1")).toEqual(profile);
    expect(daemonStore.chats.get("chat-1")).toEqual(chat);
    expect(daemonStore.chatTranscripts.get("chat-1")).toEqual(transcript);
    expect(daemonStore.chatAgentLinks.get("chat-1")).toEqual([]);
    expect(daemonStore.chatWorkspaceLinks.get("chat-1")).toEqual([]);
    expect(daemonStore.agentTraces.get("session-1")).toEqual(trace);
    expect(daemonStore.chatSummaries.get("chat-1")).toEqual(metrics);
    expect("token" in daemonStore).toBe(false);
  });
});
