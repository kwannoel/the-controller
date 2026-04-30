import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, waitFor } from "@testing-library/svelte";
import { get } from "svelte/store";
import { focusTarget, hotkeyAction } from "$lib/stores";
import type { AgentProfile } from "../daemon/types";

const sendMessage = vi.hoisted(() => vi.fn(async () => ({ seq: 1 })));
const sendChatMessage = vi.hoisted(() => vi.fn(async () => ({
  message: {
    id: "msg-1",
    chat_id: "chat-1",
    idempotency_id: null,
    body: "hello",
    token_spans: [],
    created_at: 1,
  },
    turns: [],
})));
const profiles = vi.hoisted(() => new Map());
vi.mock("../daemon/store.svelte", () => ({
  daemonStore: {
    client: { sendMessage, sendChatMessage },
    activeChatId: "chat-1",
    chats: new Map([["chat-1", { id: "chat-1", project_id: "proj-1", title: "New chat" }]]),
    chatTranscripts: new Map(),
    profiles,
  } as any,
}));

import ChatInput from "./ChatInput.svelte";

function makeProfile(overrides: Partial<AgentProfile> = {}): AgentProfile {
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
    ...overrides,
  };
}

describe("ChatInput", () => {
  beforeEach(() => {
    sendMessage.mockClear();
    sendChatMessage.mockClear();
    profiles.clear();
    profiles.set("profile-1", makeProfile());
    hotkeyAction.set(null);
    focusTarget.set(null);
  });

  it("Cmd+Enter sends user_text and clears textarea", async () => {
    const { getByRole } = render(ChatInput, { sessionId: "s1", status: "running", statusState: "idle" });
    const ta = getByRole("textbox") as HTMLTextAreaElement;
    await fireEvent.input(ta, { target: { value: "hello" } });
    await fireEvent.keyDown(ta, { key: "Enter", metaKey: true });
    expect(sendMessage).toHaveBeenCalledWith("s1", { kind: "user_text", text: "hello" });
    expect(ta.value).toBe("");
  });

  it("Cmd+Enter validates chat messages without a selected agent token", async () => {
    const { getByRole } = render(ChatInput, { chatId: "chat-1", status: "running", statusState: "idle" });
    const ta = getByRole("textbox") as HTMLTextAreaElement;
    await fireEvent.input(ta, { target: { value: "hello" } });
    await fireEvent.keyDown(ta, { key: "Enter", metaKey: true });
    expect(sendChatMessage).not.toHaveBeenCalled();
    expect(getByRole("alert").textContent).toContain("agent");
    expect(ta.value).toBe("hello");
  });

  it("selects a route token suggestion and sends it with an idempotency id", async () => {
    const { getByRole, findByRole } = render(ChatInput, { chatId: "chat-1", status: "running", statusState: "idle" });
    const ta = getByRole("textbox") as HTMLTextAreaElement;

    await fireEvent.input(ta, { target: { value: "ask %rev" } });
    await fireEvent.click(await findByRole("option", { name: /%reviewer/i }));
    await fireEvent.keyDown(ta, { key: "Enter", metaKey: true });

    expect(sendChatMessage).toHaveBeenCalledWith("chat-1", {
      body: "ask %reviewer",
      tokens: [{ kind: "shadow", handle: "reviewer", start: 4, end: 13 }],
      idempotency_id: expect.any(String),
    });
    expect(ta.value).toBe("");
  });

  it("selects a route token suggestion with keyboard navigation from the textarea", async () => {
    profiles.set("profile-2", makeProfile({
      id: "profile-2",
      handle: "zdebugger",
      name: "Debugger",
    }));
    const { getByRole } = render(ChatInput, { chatId: "chat-1", status: "running", statusState: "idle" });
    const ta = getByRole("textbox") as HTMLTextAreaElement;

    ta.value = "ask @";
    ta.setSelectionRange(5, 5);
    await fireEvent.input(ta);
    await fireEvent.keyDown(ta, { key: "ArrowDown" });
    await fireEvent.keyDown(ta, { key: "Enter" });
    await waitFor(() => expect(ta.value).toBe("ask @zdebugger"));
    await fireEvent.keyDown(ta, { key: "Enter", metaKey: true });

    expect(sendChatMessage).toHaveBeenCalledWith("chat-1", {
      body: "ask @zdebugger",
      tokens: [{ kind: "reusable", handle: "zdebugger", start: 4, end: 14 }],
      idempotency_id: expect.any(String),
    });
  });

  it("sends chat follow-ups when the chat already has an associated agent", async () => {
    const { getByRole } = render(ChatInput, {
      chatId: "chat-1",
      status: "running",
      statusState: "idle",
      hasAssociatedAgent: true,
    });
    const ta = getByRole("textbox") as HTMLTextAreaElement;

    await fireEvent.input(ta, { target: { value: "follow up" } });
    await fireEvent.keyDown(ta, { key: "Enter", metaKey: true });

    expect(sendChatMessage).toHaveBeenCalledWith("chat-1", {
      body: "follow up",
      tokens: [],
      idempotency_id: expect.any(String),
    });
  });

  it("does not send a stale selected token after the token text is removed", async () => {
    const { getByRole, findByRole } = render(ChatInput, { chatId: "chat-1", status: "running", statusState: "idle" });
    const ta = getByRole("textbox") as HTMLTextAreaElement;

    await fireEvent.input(ta, { target: { value: "ask %rev" } });
    await fireEvent.click(await findByRole("option", { name: /%reviewer/i }));
    await waitFor(() => expect(ta.value).toBe("ask %reviewer"));
    await fireEvent.input(ta, { target: { value: "ask without a token" } });
    await fireEvent.keyDown(ta, { key: "Enter", metaKey: true });

    expect(sendChatMessage).not.toHaveBeenCalled();
    expect(getByRole("alert").textContent).toContain("agent");
  });

  it("aligns token offsets with the trimmed chat body", async () => {
    const { getByRole, findByRole } = render(ChatInput, { chatId: "chat-1", status: "running", statusState: "idle" });
    const ta = getByRole("textbox") as HTMLTextAreaElement;

    ta.value = "  ask @rev";
    ta.setSelectionRange(10, 10);
    await fireEvent.input(ta);
    await fireEvent.click(await findByRole("option", { name: /@reviewer/i }));
    await fireEvent.keyDown(ta, { key: "Enter", metaKey: true });

    expect(sendChatMessage).toHaveBeenCalledWith("chat-1", {
      body: "ask @reviewer",
      tokens: [{ kind: "reusable", handle: "reviewer", start: 4, end: 13 }],
      idempotency_id: expect.any(String),
    });
  });

  it("reuses the same idempotency id when retrying an unchanged failed draft", async () => {
    sendChatMessage.mockRejectedValueOnce(new Error("lost response"));
    const { getByRole, findByRole } = render(ChatInput, { chatId: "chat-1", status: "running", statusState: "idle" });
    const ta = getByRole("textbox") as HTMLTextAreaElement;

    await fireEvent.input(ta, { target: { value: "ask %rev" } });
    await fireEvent.click(await findByRole("option", { name: /%reviewer/i }));
    await fireEvent.keyDown(ta, { key: "Enter", metaKey: true });
    await waitFor(() => expect(sendChatMessage).toHaveBeenCalledTimes(1));
    await fireEvent.keyDown(ta, { key: "Enter", metaKey: true });

    expect(sendChatMessage).toHaveBeenCalledTimes(2);
    const firstPayload = (sendChatMessage.mock.calls[0] as any[])[1];
    const secondPayload = (sendChatMessage.mock.calls[1] as any[])[1];
    const firstId = firstPayload.idempotency_id;
    const secondId = secondPayload.idempotency_id;
    expect(secondId).toBe(firstId);
  });

  it("Ctrl+Enter also sends user_text", async () => {
    const { getByRole } = render(ChatInput, { sessionId: "s1", status: "running", statusState: "idle" });
    const ta = getByRole("textbox") as HTMLTextAreaElement;
    await fireEvent.input(ta, { target: { value: "hi" } });
    await fireEvent.keyDown(ta, { key: "Enter", ctrlKey: true });
    expect(sendMessage).toHaveBeenCalledWith("s1", { kind: "user_text", text: "hi" });
  });

  it("Enter alone inserts a newline (does not send)", async () => {
    const { getByRole } = render(ChatInput, { sessionId: "s1", status: "running", statusState: "idle" });
    const ta = getByRole("textbox") as HTMLTextAreaElement;
    await fireEvent.input(ta, { target: { value: "line1" } });
    await fireEvent.keyDown(ta, { key: "Enter" });
    // sendMessage not called
    expect(sendMessage).not.toHaveBeenCalled();
  });

  it("Esc blurs the composer without interrupting when statusState is 'working'", async () => {
    const { getByRole } = render(ChatInput, { sessionId: "s1", status: "running", statusState: "working" });
    const ta = getByRole("textbox") as HTMLTextAreaElement;
    ta.focus();

    await fireEvent.keyDown(ta, { key: "Escape" });

    expect(sendMessage).not.toHaveBeenCalled();
    expect(document.activeElement).not.toBe(ta);
  });

  it("Esc focuses the active chat row when a chat is active", async () => {
    const { getByRole } = render(ChatInput, { chatId: "chat-1", status: "running", statusState: "idle" });
    const ta = getByRole("textbox") as HTMLTextAreaElement;
    ta.focus();

    await fireEvent.keyDown(ta, { key: "Escape" });

    expect(get(focusTarget)).toEqual({ type: "chat", chatId: "chat-1", projectId: "proj-1" });
  });

  it("Shift+Esc triggers interrupt when statusState is 'working'", async () => {
    const { getByRole } = render(ChatInput, { sessionId: "s1", status: "running", statusState: "working" });
    const ta = getByRole("textbox") as HTMLTextAreaElement;
    await fireEvent.keyDown(ta, { key: "Escape", shiftKey: true });
    expect(sendMessage).toHaveBeenCalledWith("s1", { kind: "interrupt" });
  });

  it("Esc does NOT trigger interrupt when idle", async () => {
    const { getByRole } = render(ChatInput, { sessionId: "s1", status: "running", statusState: "idle" });
    const ta = getByRole("textbox") as HTMLTextAreaElement;
    await fireEvent.keyDown(ta, { key: "Escape" });
    expect(sendMessage).not.toHaveBeenCalled();
  });

  it("Shift+Esc does NOT trigger interrupt when idle", async () => {
    const { getByRole } = render(ChatInput, { sessionId: "s1", status: "running", statusState: "idle" });
    const ta = getByRole("textbox") as HTMLTextAreaElement;
    await fireEvent.keyDown(ta, { key: "Escape", shiftKey: true });
    expect(sendMessage).not.toHaveBeenCalled();
  });

  it("Esc blurs the composer when idle", async () => {
    const { getByRole } = render(ChatInput, { sessionId: "s1", status: "running", statusState: "idle" });
    const ta = getByRole("textbox") as HTMLTextAreaElement;
    ta.focus();
    expect(document.activeElement).toBe(ta);

    await fireEvent.keyDown(ta, { key: "Escape" });

    expect(document.activeElement).not.toBe(ta);
  });

  it("focus-chat-input hotkey action focuses the composer", async () => {
    const { getByRole } = render(ChatInput, { sessionId: "s1", status: "running", statusState: "idle" });
    const ta = getByRole("textbox") as HTMLTextAreaElement;

    hotkeyAction.set({ type: "focus-chat-input" });

    await waitFor(() => expect(document.activeElement).toBe(ta));
  });

  it("Textarea is disabled when session status is 'ended'", () => {
    const { getByRole } = render(ChatInput, { sessionId: "s1", status: "ended", statusState: null });
    const ta = getByRole("textbox") as HTMLTextAreaElement;
    expect(ta.disabled).toBe(true);
  });

  it("Does not send when textarea is empty", async () => {
    const { getByRole } = render(ChatInput, { sessionId: "s1", status: "running", statusState: "idle" });
    const ta = getByRole("textbox") as HTMLTextAreaElement;
    await fireEvent.keyDown(ta, { key: "Enter", metaKey: true });
    expect(sendMessage).not.toHaveBeenCalled();
  });
});
