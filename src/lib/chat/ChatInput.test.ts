import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, waitFor } from "@testing-library/svelte";
import { hotkeyAction } from "$lib/stores";

const sendMessage = vi.hoisted(() => vi.fn(async () => ({ seq: 1 })));
vi.mock("../daemon/store.svelte", () => ({
  daemonStore: { client: { sendMessage } } as any,
}));

import ChatInput from "./ChatInput.svelte";

describe("ChatInput", () => {
  beforeEach(() => {
    sendMessage.mockClear();
    hotkeyAction.set(null);
  });

  it("Cmd+Enter sends user_text and clears textarea", async () => {
    const { getByRole } = render(ChatInput, { sessionId: "s1", status: "running", statusState: "idle" });
    const ta = getByRole("textbox") as HTMLTextAreaElement;
    await fireEvent.input(ta, { target: { value: "hello" } });
    await fireEvent.keyDown(ta, { key: "Enter", metaKey: true });
    expect(sendMessage).toHaveBeenCalledWith("s1", { kind: "user_text", text: "hello" });
    expect(ta.value).toBe("");
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
