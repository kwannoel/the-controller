import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent } from "@testing-library/svelte";

const { createSession } = vi.hoisted(() => ({ createSession: vi.fn() }));
vi.mock("../daemon/store.svelte", () => ({
  daemonStore: { client: { createSession }, activeSessionId: null, newChatTarget: null } as any,
}));

import NewChatDialog from "./NewChatDialog.svelte";

describe("NewChatDialog", () => {
  beforeEach(() => vi.clearAllMocks());

  it("submits agent + cwd + initial prompt and closes", async () => {
    createSession.mockResolvedValueOnce({ id: "s1", label: "session-1" });
    const onClose = vi.fn();
    const { getByLabelText, getByText } = render(NewChatDialog, { projectCwd: "/tmp/a", onClose });
    await fireEvent.change(getByLabelText("Agent"), { target: { value: "claude" } });
    await fireEvent.input(getByLabelText("Initial prompt"), { target: { value: "hi" } });
    await fireEvent.click(getByText("Create"));
    expect(createSession).toHaveBeenCalledWith({ agent: "claude", cwd: "/tmp/a", initial_prompt: "hi" });
    expect(onClose).toHaveBeenCalled();
  });

  it("shows inline error on 422", async () => {
    const err: any = new Error("boom");
    err.name = "DaemonHttpError";
    err.status = 422;
    createSession.mockRejectedValueOnce(err);
    const { getByLabelText, getByText, findByText } = render(NewChatDialog, { projectCwd: "/tmp/a", onClose: () => {} });
    await fireEvent.change(getByLabelText("Agent"), { target: { value: "claude" } });
    await fireEvent.click(getByText("Create"));
    expect(await findByText(/Agent binary not configured/)).toBeTruthy();
  });

  it("submit disabled until agent selected", async () => {
    const { getByText } = render(NewChatDialog, { projectCwd: "/tmp/a", onClose: () => {} });
    const btn = getByText("Create") as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
  });
});
