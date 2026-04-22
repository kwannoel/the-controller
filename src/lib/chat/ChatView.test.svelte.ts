import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, waitFor } from "@testing-library/svelte";

const openStreamMock = vi.hoisted(() => vi.fn(() => ({ close: vi.fn() })));

vi.mock("../daemon/store.svelte", () => import("./__mocks__/daemonStore.svelte"));
vi.mock("../daemon/stream", () => ({ openStream: openStreamMock }));

import ChatView from "./ChatView.svelte";
import { readEvents } from "./__mocks__/daemonStore.svelte";

describe("ChatView", () => {
  beforeEach(() => vi.clearAllMocks());

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
});
