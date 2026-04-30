import { describe, it, expect, vi, beforeEach } from "vitest";

class MockWebSocket {
  static instances: MockWebSocket[] = [];
  static clear() { this.instances = []; }
  onopen: ((e: any) => void) | null = null;
  onmessage: ((e: any) => void) | null = null;
  onclose: ((e: any) => void) | null = null;
  onerror: ((e: any) => void) | null = null;
  readyState = 0;
  constructor(public url: string) { MockWebSocket.instances.push(this); }
  send() {}
  close() { this.readyState = 3; this.onclose?.({ code: 1006 }); }
}

describe("stream", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    MockWebSocket.clear();
    vi.resetModules();
    vi.stubGlobal("WebSocket", MockWebSocket as any);
    vi.doMock("$lib/backend", () => ({
      command: vi.fn(async (cmd: string) => {
        throw new Error("unexpected command " + cmd);
      }),
      listen: () => () => {},
    }));
  });

  it("reconnects with since=<lastSeq>", async () => {
    const { daemonStore } = await import("./store.svelte");
    const { DaemonClient } = await import("./client");
    daemonStore.client = new DaemonClient("/api/daemon");
    daemonStore.transcripts.set("s1", { events: [], lastSeq: 5, inProgressBlocks: new Map(), statusState: null, tokenUsage: null, seenSeq: new Set() });
    const { openStream } = await import("./stream");
    const handle = openStream("s1");
    expect(MockWebSocket.instances.length).toBe(1);
    expect(MockWebSocket.instances[0].url).toBe("/api/daemon/sessions/s1/stream?since=5");
    MockWebSocket.instances[0].close();
    await vi.advanceTimersByTimeAsync(1100);
    expect(MockWebSocket.instances.length).toBe(2);
    expect(MockWebSocket.instances[1].url).toBe("/api/daemon/sessions/s1/stream?since=5");
    handle.close();
  });

  it("close() prevents further reconnects", async () => {
    const { daemonStore } = await import("./store.svelte");
    const { DaemonClient } = await import("./client");
    daemonStore.client = new DaemonClient("/api/daemon");
    daemonStore.transcripts.set("s2", { events: [], lastSeq: 0, inProgressBlocks: new Map(), statusState: null, tokenUsage: null, seenSeq: new Set() });
    const { openStream } = await import("./stream");
    const handle = openStream("s2");
    handle.close();
    await vi.advanceTimersByTimeAsync(1100);
    expect(MockWebSocket.instances.length).toBe(1);
  });

  it("incoming event is reduced into the transcript", async () => {
    const { daemonStore } = await import("./store.svelte");
    const { DaemonClient } = await import("./client");
    daemonStore.client = new DaemonClient("/api/daemon");
    daemonStore.transcripts.set("s3", { events: [], lastSeq: 0, inProgressBlocks: new Map(), statusState: null, tokenUsage: null, seenSeq: new Set() });
    const { openStream } = await import("./stream");
    const handle = openStream("s3");
    const ws = MockWebSocket.instances[0];
    ws.onmessage?.({
      data: JSON.stringify({
        session_id: "s3", seq: 1, channel: "inbox", kind: "user_text",
        payload: { text: "hi" }, created_at: 1, applied_at: null,
      }),
    });
    const t = daemonStore.transcripts.get("s3")!;
    expect(t.events.length).toBe(1);
    expect(t.lastSeq).toBe(1);
    handle.close();
  });
});
