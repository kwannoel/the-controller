import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, waitFor } from "@testing-library/svelte";
import { get } from "svelte/store";
import { workspaceMode } from "../stores";

const openStreamMock = vi.hoisted(() => vi.fn(() => ({ close: vi.fn() })));

vi.mock("../daemon/store.svelte", () => import("./__mocks__/daemonStore.svelte"));
vi.mock("../daemon/stream", () => ({ openStream: openStreamMock }));

import ChatView from "./ChatView.svelte";
import {
  daemonStore,
  loadAgentTrace,
  loadChatMetrics,
  readChatTranscript,
  readEvents,
  sendChatMessage,
} from "./__mocks__/daemonStore.svelte";

function makeChat(id: string, title: string) {
  return {
    id,
    project_id: "proj-1",
    title,
    created_at: 1,
    updated_at: 1,
    deleted_at: null,
  };
}

function makeChatMessage(id: string, chatId: string, body: string) {
  return {
    id,
    chat_id: chatId,
    idempotency_id: null,
    body,
    token_spans: [],
    created_at: 1,
  };
}

function makeProfile() {
  return {
    id: "profile-1",
    handle: "reviewer",
    name: "Reviewer",
    description: "Reviews changes",
    runtime: "codex",
    skills: [],
    prompt: "Review changes.",
    archived_at: null,
    avatar_asset_path: null,
    avatar_status: "pending",
    avatar_error: null,
    active_version_id: "version-1",
    created_at: 1,
    updated_at: 1,
  };
}

function makeWorkspaceLink() {
  return {
    id: "workspace-link-1",
    chat_id: "chat-1",
    project_id: "proj-1",
    workspace_id: "workspace-1",
    path: "/repo/controller",
    label: "controller-routing",
    branch: "codex/routing",
    focused: true,
    created_at: 1,
    updated_at: 1,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("ChatView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    daemonStore.client = {
      readEvents,
      readChatTranscript,
      sendChatMessage,
      wsUrl: () => "ws://x",
      chatStreamUrl: () => "ws://chat",
    };
    daemonStore.sessions.clear();
    daemonStore.sessions.set("s1", { id: "s1", label: "Chat 1", agent: "claude", status: "running" });
    daemonStore.transcripts.clear();
    daemonStore.activeSessionId = "s1";
    daemonStore.chats.clear();
    daemonStore.chatTranscripts.clear();
    daemonStore.chatAgentLinks.clear();
    daemonStore.chatWorkspaceLinks.clear();
    daemonStore.chatSummaries.clear();
    daemonStore.agentTraces.clear();
    daemonStore.profiles.clear();
    daemonStore.profiles.set("profile-1", makeProfile());
    daemonStore.activeChatId = null;
    daemonStore.activeSessionId = null;
    workspaceMode.set("chat");
    readChatTranscript.mockResolvedValue([]);
    sendChatMessage.mockResolvedValue({
      message: makeChatMessage("msg-1", "chat-1", "hello"),
      turns: [],
    });
  });

  it("hydrates from readEvents and renders the agent text", async () => {
    readEvents.mockResolvedValueOnce([
      { session_id: "s1", seq: 1, channel: "outbox", kind: "agent_text",
        payload: { block_id: "b1", message_id: "m1", text: "hello" }, created_at: 1, applied_at: null },
    ]);
    const { findByText } = render(ChatView, { sessionId: "s1" });
    expect(await findByText("hello")).toBeTruthy();
  });

  it("opens a stream for the session on mount", async () => {
    readEvents.mockResolvedValueOnce([]);
    render(ChatView, { sessionId: "s1" });
    await waitFor(() => expect(openStreamMock).toHaveBeenCalledWith("s1"));
  });

  it("loads a new chat transcript when the chat id changes", async () => {
    daemonStore.chats.set("chat-a", makeChat("chat-a", "Chat A"));
    daemonStore.chats.set("chat-b", makeChat("chat-b", "Chat B"));

    const { rerender } = render(ChatView, { chatId: "chat-a" });
    await waitFor(() => expect(readChatTranscript).toHaveBeenCalledWith("chat-a"));

    await rerender({ chatId: "chat-b" });

    await waitFor(() => expect(readChatTranscript).toHaveBeenCalledWith("chat-b"));
  });

  it("keeps optimistic chat messages when the initial transcript read resolves later", async () => {
    const initialRead = deferred<any[]>();
    readChatTranscript.mockReturnValueOnce(initialRead.promise);
    sendChatMessage.mockResolvedValueOnce({
      message: makeChatMessage("msg-optimistic", "chat-1", "ask %reviewer optimistic hello"),
      turns: [],
    });
    daemonStore.chats.set("chat-1", makeChat("chat-1", "Chat 1"));

    const { findByRole, findByText, getByRole } = render(ChatView, { chatId: "chat-1" });
    const ta = getByRole("textbox") as HTMLTextAreaElement;

    await fireEvent.input(ta, { target: { value: "ask %rev optimistic hello" } });
    ta.setSelectionRange(8, 8);
    await fireEvent.keyUp(ta, { key: "v" });
    await fireEvent.click(await findByRole("option", { name: /%reviewer/i }));
    await fireEvent.keyDown(ta, { key: "Enter", metaKey: true });
    expect(await findByText("ask %reviewer optimistic hello")).toBeTruthy();

    initialRead.resolve([]);

    await waitFor(() => expect(readChatTranscript).toHaveBeenCalledWith("chat-1"));
    expect(await findByText("ask %reviewer optimistic hello")).toBeTruthy();
  });

  it("renders the chat summary from known agent and workspace links", async () => {
    daemonStore.chats.set("chat-1", makeChat("chat-1", "Chat 1"));
    daemonStore.profiles.set("profile-2", {
      ...makeProfile(),
      id: "profile-2",
      handle: "debugger",
      name: "Debugger",
    });
    daemonStore.chatTranscripts.set("chat-1", [
      {
        type: "user_message",
        message: {
          ...makeChatMessage("msg-1", "chat-1", "ask @reviewer %debugger"),
          token_spans: [
            { kind: "reusable", handle: "reviewer", start: 4, end: 13 },
            { kind: "shadow", handle: "debugger", start: 14, end: 23 },
          ],
        },
      },
    ]);
    daemonStore.chatWorkspaceLinks.set("chat-1", [makeWorkspaceLink()]);

    const { findByText } = render(ChatView, { chatId: "chat-1" });

    expect(await findByText("@reviewer")).toBeTruthy();
    expect(await findByText("%debugger")).toBeTruthy();
    expect(await findByText("focused @reviewer")).toBeTruthy();
    expect(await findByText("controller-routing")).toBeTruthy();
    expect(await findByText("focused controller-routing")).toBeTruthy();
  });

  it("renders linked agents from known chat agent links", async () => {
    daemonStore.chats.set("chat-1", makeChat("chat-1", "Chat 1"));
    daemonStore.chatAgentLinks.set("chat-1", [
      {
        id: "agent-link-1",
        chat_id: "chat-1",
        session_id: "session-1",
        profile_id: "profile-1",
        profile_version_id: "version-1",
        route_type: "reusable",
        focused: true,
        token_source: "@reviewer",
        created_at: 1,
      },
    ]);

    const { findByText } = render(ChatView, { chatId: "chat-1" });

    expect(await findByText("@reviewer")).toBeTruthy();
    expect(await findByText("focused @reviewer")).toBeTruthy();
  });

  it("marks the latest reusable token as the focused summary agent", async () => {
    daemonStore.chats.set("chat-1", makeChat("chat-1", "Chat 1"));
    daemonStore.profiles.set("profile-2", {
      ...makeProfile(),
      id: "profile-2",
      handle: "planner",
      name: "Planner",
    });
    daemonStore.chatTranscripts.set("chat-1", [
      {
        type: "user_message",
        message: {
          ...makeChatMessage("msg-1", "chat-1", "ask @planner @reviewer"),
          token_spans: [
            { kind: "reusable", handle: "planner", start: 4, end: 12 },
            { kind: "reusable", handle: "reviewer", start: 13, end: 22 },
          ],
        },
      },
    ]);

    const { findByText, queryByText } = render(ChatView, { chatId: "chat-1" });

    expect(await findByText("focused @reviewer")).toBeTruthy();
    expect(queryByText("focused @planner")).toBeNull();
  });

  it("loads chat metrics and opens an agent trace from the metrics tab", async () => {
    daemonStore.chats.set("chat-1", makeChat("chat-1", "Chat 1"));
    daemonStore.chatAgentLinks.set("chat-1", [
      {
        id: "agent-link-1",
        chat_id: "chat-1",
        session_id: "session-1",
        profile_id: "profile-1",
        profile_version_id: "version-1",
        route_type: "reusable",
        focused: true,
        token_source: "@reviewer",
        created_at: 1,
      },
    ]);
    daemonStore.sessions.set("session-1", {
      id: "session-1",
      label: "Reviewer",
      agent: "codex",
      cwd: "/repo",
      args: [],
      status: "running",
      native_session_id: null,
      pid: null,
      created_at: 1,
      updated_at: 1,
      ended_at: null,
      end_reason: null,
    });
    daemonStore.chatSummaries.set("chat-1", {
      chat_id: "chat-1",
      turn_count: 1,
      input_tokens: 0,
      output_tokens: 20,
      cache_read_tokens: 0,
      cache_write_tokens: 0,
      tool_call_count: 1,
      outbox_write_count: 1,
      error_count: 0,
      updated_at: 1,
    });
    daemonStore.agentTraces.set("session-1", [
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
    ]);

    const { findByRole, findAllByText, getByRole } = render(ChatView, { chatId: "chat-1" });
    await fireEvent.click(await findByRole("tab", { name: "Metrics" }));

    await waitFor(() => expect(loadChatMetrics).toHaveBeenCalledWith("chat-1"));
    await waitFor(() => expect(loadAgentTrace).toHaveBeenCalledWith("session-1"));
    expect((await findAllByText("20 tokens")).length).toBeGreaterThan(0);

    await fireEvent.click(getByRole("button", { name: /open reviewer trace/i }));

    expect(daemonStore.activeSessionId).toBe("session-1");
    expect(get(workspaceMode)).toBe("agent-observe");
  });
});
