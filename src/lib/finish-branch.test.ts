import { describe, expect, it, vi } from "vitest";
import { sendFinishBranchPrompt } from "./finish-branch";

describe("sendFinishBranchPrompt", () => {
  it("sends Codex merge text first and Enter through the raw PTY path", async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);

    await sendFinishBranchPrompt(invoke, "sess-1", "codex");

    expect(invoke).toHaveBeenNthCalledWith(1, "write_to_pty", {
      sessionId: "sess-1",
      data: "$the-controller-finishing-a-development-branch",
    });
    expect(invoke).toHaveBeenNthCalledWith(2, "send_raw_to_pty", {
      sessionId: "sess-1",
      data: "\r",
    });
  });

  it("keeps Claude merge submission as a single PTY write", async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);

    await sendFinishBranchPrompt(invoke, "sess-1", "claude");

    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith("write_to_pty", {
      sessionId: "sess-1",
      data: "/the-controller-finishing-a-development-branch\r",
    });
  });
});
