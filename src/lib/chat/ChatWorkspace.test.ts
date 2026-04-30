import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent } from "@testing-library/svelte";

vi.mock("../daemon/store.svelte", () => {
  const daemonStore = {
    reachable: false,
    client: null as any,
    sessions: new Map(),
    transcripts: new Map(),
    activeSessionId: null as string | null,
  };
  return {
    daemonStore,
    bootstrap: vi.fn(async () => {}),
    pingDaemon: vi.fn(async () => {}),
    loadSessions: vi.fn(async () => {}),
  };
});

describe("ChatWorkspace", () => {
  beforeEach(() => vi.clearAllMocks());

  it("renders empty state when daemon unreachable", async () => {
    const { daemonStore } = await import("../daemon/store.svelte");
    (daemonStore as any).reachable = false;
    const ChatWorkspace = (await import("./ChatWorkspace.svelte")).default;
    const { findByText, queryByText } = render(ChatWorkspace);
    expect(await findByText(/Daemon not running/)).toBeTruthy();
    expect(await findByText(/\/api\/daemon/)).toBeTruthy();
    expect(queryByText(/Expected token|daemon\.token/)).toBeNull();
  });

  it("clicking Retry calls pingDaemon", async () => {
    const { daemonStore, pingDaemon } = await import("../daemon/store.svelte");
    (daemonStore as any).reachable = false;
    const ChatWorkspace = (await import("./ChatWorkspace.svelte")).default;
    const { getByText } = render(ChatWorkspace);
    await fireEvent.click(getByText("Retry"));
    expect(pingDaemon).toHaveBeenCalled();
  });
});
