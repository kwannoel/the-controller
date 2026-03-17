import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { get } from "svelte/store";

// Opt out of the global $lib/backend mock so we can test the real implementation.
vi.unmock("$lib/backend");

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("backend adapter", () => {
  beforeEach(() => {
    vi.resetModules();
    delete (window as any).__TAURI_INTERNALS__;
    sessionStorage.clear();
  });

  it("should use invoke when __TAURI_INTERNALS__ is present", async () => {
    (window as any).__TAURI_INTERNALS__ = {};
    const { command } = await import("./backend");
    const { invoke } = await import("@tauri-apps/api/core");
    (invoke as any).mockResolvedValue({ id: "123" });

    const result = await command("list_projects");
    expect(invoke).toHaveBeenCalledWith("list_projects", undefined);
    expect(result).toEqual({ id: "123" });
  });

  it("should use fetch when __TAURI_INTERNALS__ is absent", async () => {
    const mockResponse = { id: "456" };
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(mockResponse),
      text: () => Promise.resolve(""),
    });

    const { command } = await import("./backend");
    const result = await command("create_project", { name: "test" });

    expect(fetch).toHaveBeenCalledWith("/api/create_project", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name: "test" }),
    });
    expect(result).toEqual(mockResponse);
  });

  it("should throw on non-ok fetch response", async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: false,
      text: () => Promise.resolve("not found"),
    });

    const { command } = await import("./backend");
    await expect(command("bad_command")).rejects.toThrow("not found");
  });

  it("should read token from URL, store in sessionStorage, and strip from URL", async () => {
    history.replaceState(null, "", "/?token=secret123");

    await import("./backend");

    expect(sessionStorage.getItem("authToken")).toBe("secret123");
    expect(window.location.search).not.toContain("token");
  });

  it("should send Authorization header when auth token is set", async () => {
    sessionStorage.setItem("authToken", "mytoken");
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ ok: true }),
      text: () => Promise.resolve(""),
    });

    const { command } = await import("./backend");
    await command("some_command", { key: "val" });

    expect(fetch).toHaveBeenCalledWith("/api/some_command", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: "Bearer mytoken",
      },
      body: JSON.stringify({ key: "val" }),
    });
  });

  it("should set authError and throw on 401 response", async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: false,
      status: 401,
      text: () => Promise.resolve("Unauthorized"),
    });

    const { command, authError } = await import("./backend");
    await expect(command("protected_command")).rejects.toThrow("Unauthorized");
    expect(get(authError)).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// WebSocket / listen() tests
// ---------------------------------------------------------------------------

class MockWebSocket {
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;

  CONNECTING = 0;
  OPEN = 1;
  CLOSING = 2;
  CLOSED = 3;

  readyState = MockWebSocket.OPEN;
  url: string;
  private handlers: Record<string, Array<(ev: any) => void>> = {};

  constructor(url: string) {
    this.url = url;
    // Auto-fire "open" asynchronously so listeners registered after construction see it
    queueMicrotask(() => this.emit("open", {}));
  }

  addEventListener(type: string, cb: (ev: any) => void) {
    (this.handlers[type] ??= []).push(cb);
  }

  removeEventListener(type: string, cb: (ev: any) => void) {
    this.handlers[type] = (this.handlers[type] ?? []).filter((h) => h !== cb);
  }

  emit(type: string, ev: any) {
    for (const h of this.handlers[type] ?? []) h(ev);
  }

  close() {
    this.readyState = MockWebSocket.CLOSED;
  }
}

describe("WebSocket listen()", () => {
  let originalWebSocket: typeof WebSocket;
  let instances: MockWebSocket[];

  beforeEach(() => {
    vi.resetModules();
    vi.useFakeTimers();
    delete (window as any).__TAURI_INTERNALS__;
    sessionStorage.clear();

    instances = [];
    originalWebSocket = globalThis.WebSocket;
    (globalThis as any).WebSocket = class extends MockWebSocket {
      constructor(url: string) {
        super(url);
        instances.push(this);
      }
    };
    // Assign static constants expected by the production code
    (globalThis as any).WebSocket.CLOSED = MockWebSocket.CLOSED;
    (globalThis as any).WebSocket.CLOSING = MockWebSocket.CLOSING;
    (globalThis as any).WebSocket.OPEN = MockWebSocket.OPEN;
    (globalThis as any).WebSocket.CONNECTING = MockWebSocket.CONNECTING;
  });

  afterEach(() => {
    vi.useRealTimers();
    globalThis.WebSocket = originalWebSocket;
  });

  it("should dispatch matching events to the handler", async () => {
    const { listen } = await import("./backend");
    const handler = vi.fn();
    listen("my-event", handler);

    // Wait for the microtask (open event)
    await vi.advanceTimersByTimeAsync(0);

    const ws = instances[0];
    ws.emit("message", { data: JSON.stringify({ event: "my-event", payload: { x: 1 } }) });
    expect(handler).toHaveBeenCalledWith({ x: 1 });
  });

  it("should not dispatch events that don't match", async () => {
    const { listen } = await import("./backend");
    const handler = vi.fn();
    listen("my-event", handler);

    await vi.advanceTimersByTimeAsync(0);

    const ws = instances[0];
    ws.emit("message", { data: JSON.stringify({ event: "other-event", payload: {} }) });
    expect(handler).not.toHaveBeenCalled();
  });

  it("should set authError on close code 1008 (Policy Violation)", async () => {
    const { listen, authError } = await import("./backend");
    listen("any-event", vi.fn());

    await vi.advanceTimersByTimeAsync(0);

    const ws = instances[0];
    ws.readyState = MockWebSocket.CLOSED;
    ws.emit("close", { code: 1008 });

    expect(get(authError)).toBe(true);
  });

  it("should suppress reconnect after auth failure (code 1008)", async () => {
    const { listen } = await import("./backend");
    listen("any-event", vi.fn());

    await vi.advanceTimersByTimeAsync(0);
    expect(instances).toHaveLength(1);

    const ws = instances[0];
    ws.readyState = MockWebSocket.CLOSED;
    ws.emit("close", { code: 1008 });

    // Advance past the reconnect delay — no new WebSocket should be created
    await vi.advanceTimersByTimeAsync(5000);
    expect(instances).toHaveLength(1);
  });

  it("should reconnect on normal close (non-auth)", async () => {
    const { listen } = await import("./backend");
    listen("any-event", vi.fn());

    await vi.advanceTimersByTimeAsync(0);
    expect(instances).toHaveLength(1);

    const ws = instances[0];
    ws.readyState = MockWebSocket.CLOSED;
    ws.emit("close", { code: 1006 }); // abnormal closure, not auth

    // Advance past the reconnect delay
    await vi.advanceTimersByTimeAsync(1500);
    expect(instances).toHaveLength(2);
  });

  it("should unsubscribe when the returned cleanup function is called", async () => {
    const { listen } = await import("./backend");
    const handler = vi.fn();
    const unlisten = listen("my-event", handler);

    await vi.advanceTimersByTimeAsync(0);

    unlisten();

    const ws = instances[0];
    ws.emit("message", { data: JSON.stringify({ event: "my-event", payload: {} }) });
    expect(handler).not.toHaveBeenCalled();
  });
});
