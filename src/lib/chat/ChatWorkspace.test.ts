import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, waitFor } from "@testing-library/svelte";
import { projects, focusTarget, workspaceMode, hotkeyAction, type Project } from "$lib/stores";

vi.mock("../daemon/store.svelte", () => import("./__mocks__/daemonStore.svelte"));

vi.mock("../daemon/stream", () => ({
  openStream: vi.fn(() => ({ close: vi.fn() })),
}));

function makeProject(id: string, name: string, repoPath: string): Project {
  return {
    id,
    name,
    repo_path: repoPath,
    created_at: "2026-01-01",
    archived: false,
    maintainer: { enabled: false, interval_minutes: 60 },
    auto_worker: { enabled: false },
    sessions: [],
    prompts: [],
    staged_sessions: [],
  };
}

function pressKey(key: string) {
  window.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true }));
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

describe("ChatWorkspace", () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    const { daemonStore, readChatTranscript } = await import("./__mocks__/daemonStore.svelte");
    daemonStore.reachable = true;
    daemonStore.client = {
      readEvents: vi.fn(async () => []),
      readChatTranscript,
      createChat: vi.fn(),
      sendMessage: vi.fn(),
      sendChatMessage: vi.fn(),
      wsUrl: () => "ws://x",
      chatStreamUrl: () => "ws://chat",
    };
    daemonStore.sessions.clear();
    daemonStore.transcripts.clear();
    daemonStore.activeSessionId = null;
    daemonStore.chats.clear();
    daemonStore.chatTranscripts.clear();
    daemonStore.activeChatId = null;
    daemonStore.newChatTarget = null;
    readChatTranscript.mockResolvedValue([]);
    projects.set([]);
    focusTarget.set(null);
    workspaceMode.set("chat");
    hotkeyAction.set(null);
  });

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

  it("pressing n creates a chat for the focused project and focuses the composer", async () => {
    const { daemonStore } = await import("../daemon/store.svelte");
    const createChat = vi.fn(async () => ({
      id: "chat-1",
      project_id: "proj-1",
      title: "New chat",
      created_at: 1,
      updated_at: 1,
      deleted_at: null,
    }));
    (daemonStore as any).client.createChat = createChat;
    projects.set([makeProject("proj-1", "Controller", "/tmp/controller")]);
    focusTarget.set({ type: "project", projectId: "proj-1" });

    const ChatWorkspace = (await import("./ChatWorkspace.svelte")).default;
    const HotkeyManager = (await import("../HotkeyManager.svelte")).default;
    const { queryByRole, getByRole } = render(ChatWorkspace);
    render(HotkeyManager);

    pressKey("n");

    await waitFor(() => expect(createChat).toHaveBeenCalledWith({
      project_id: "proj-1",
      title: "New chat",
    }));
    await waitFor(() => expect((daemonStore as any).activeChatId).toBe("chat-1"));
    await waitFor(() => expect(document.activeElement).toBe(getByRole("textbox")));
    expect(queryByRole("dialog")).toBeNull();
  });

  it("does not replay a second new chat request after the first create resolves", async () => {
    const { daemonStore } = await import("../daemon/store.svelte");
    const firstCreate = deferred<any>();
    const createChat = vi.fn(() => firstCreate.promise);
    (daemonStore as any).client.createChat = createChat;
    projects.set([makeProject("proj-1", "Controller", "/tmp/controller")]);
    focusTarget.set({ type: "project", projectId: "proj-1" });

    const ChatWorkspace = (await import("./ChatWorkspace.svelte")).default;
    const HotkeyManager = (await import("../HotkeyManager.svelte")).default;
    render(ChatWorkspace);
    render(HotkeyManager);

    pressKey("n");
    await waitFor(() => expect(createChat).toHaveBeenCalledTimes(1));
    pressKey("n");
    expect(createChat).toHaveBeenCalledTimes(1);

    firstCreate.resolve({
      id: "chat-1",
      project_id: "proj-1",
      title: "New chat",
      created_at: 1,
      updated_at: 1,
      deleted_at: null,
    });

    await waitFor(() => expect((daemonStore as any).activeChatId).toBe("chat-1"));
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(createChat).toHaveBeenCalledTimes(1);
  });
});
