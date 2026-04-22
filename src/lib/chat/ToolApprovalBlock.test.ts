import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent } from "@testing-library/svelte";

const sendMessage = vi.hoisted(() => vi.fn());
vi.mock("../daemon/store.svelte", () => ({
  daemonStore: { client: { sendMessage } } as any,
}));

import ToolApprovalBlock from "./ToolApprovalBlock.svelte";

describe("ToolApprovalBlock", () => {
  beforeEach(() => {
    sendMessage.mockReset();
    sendMessage.mockResolvedValue({ seq: 99 });
  });

  it("Approve click sends approved=true", async () => {
    const { getByText } = render(ToolApprovalBlock, { callId: "c1", sessionId: "s1" });
    await fireEvent.click(getByText("Approve"));
    expect(sendMessage).toHaveBeenCalledWith("s1", {
      kind: "tool_approval",
      call_id: "c1",
      approved: true,
    });
  });

  it("Deny click sends approved=false without reason", async () => {
    const { getByText } = render(ToolApprovalBlock, { callId: "c1", sessionId: "s1" });
    await fireEvent.click(getByText("Deny"));
    expect(sendMessage).toHaveBeenCalledWith("s1", {
      kind: "tool_approval",
      call_id: "c1",
      approved: false,
    });
  });

  it("Deny with reason opens textarea and includes reason", async () => {
    const { getByText, getByLabelText } = render(ToolApprovalBlock, {
      callId: "c1",
      sessionId: "s1",
    });
    await fireEvent.click(getByText("Deny with reason"));
    await fireEvent.input(getByLabelText("Deny reason"), { target: { value: "because" } });
    await fireEvent.click(getByText("Submit"));
    expect(sendMessage).toHaveBeenCalledWith("s1", {
      kind: "tool_approval",
      call_id: "c1",
      approved: false,
      reason: "because",
    });
  });

  it("buttons are disabled while request in flight", async () => {
    let resolve!: (v: any) => void;
    sendMessage.mockReturnValueOnce(
      new Promise((r) => {
        resolve = r;
      }),
    );
    const { getByText } = render(ToolApprovalBlock, { callId: "c1", sessionId: "s1" });
    const approve = getByText("Approve") as HTMLButtonElement;
    await fireEvent.click(approve);
    expect(approve.disabled).toBe(true);
    resolve({ seq: 1 });
  });
});
