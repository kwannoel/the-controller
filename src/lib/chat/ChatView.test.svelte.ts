import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, waitFor } from "@testing-library/svelte";

const openStreamMock = vi.hoisted(() => vi.fn(() => ({ close: vi.fn() })));

vi.mock("../daemon/store.svelte", () => import("./__mocks__/daemonStore.svelte"));
vi.mock("../daemon/stream", () => ({ openStream: openStreamMock }));

import ChatView from "./ChatView.svelte";
import { daemonStore, readChatTranscript, readEvents, sendChatMessage } from "./__mocks__/daemonStore.svelte";

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
    daemonStore.profiles.clear();
    daemonStore.profiles.set("profile-1", makeProfile());
    daemonStore.activeChatId = null;
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
});
