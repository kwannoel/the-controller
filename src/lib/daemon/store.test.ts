import { describe, it, expect, vi, beforeEach } from "vitest";

describe("daemon store bootstrap", () => {
  beforeEach(() => vi.resetModules());

  it("sets reachable=true on successful ping", async () => {
    vi.doMock("$lib/backend", () => ({
      command: vi.fn(async (cmd: string) => {
        if (cmd === "read_daemon_token") return "TOK";
        throw new Error("unexpected command " + cmd);
      }),
      listen: () => () => {},
    }));
    const fetchMock = vi.fn(async () => ({ ok: true, status: 200, json: async () => [] } as any));
    vi.stubGlobal("fetch", fetchMock);
    const { daemonStore, bootstrap } = await import("./store.svelte");
    await bootstrap();
    expect(daemonStore.reachable).toBe(true);
    expect(daemonStore.token).toBe("TOK");
  });

  it("sets reachable=false when ping fails", async () => {
    vi.doMock("$lib/backend", () => ({
      command: vi.fn(async () => "TOK"),
      listen: () => () => {},
    }));
    vi.stubGlobal("fetch", vi.fn(async () => { throw new TypeError("connect refused"); }));
    const { daemonStore, bootstrap } = await import("./store.svelte");
    await bootstrap();
    expect(daemonStore.reachable).toBe(false);
  });

  it("sets reachable=false when read_daemon_token fails", async () => {
    vi.doMock("$lib/backend", () => ({
      command: vi.fn(async () => { throw new Error("token missing"); }),
      listen: () => () => {},
    }));
    vi.stubGlobal("fetch", vi.fn());
    const { daemonStore, bootstrap } = await import("./store.svelte");
    await bootstrap();
    expect(daemonStore.reachable).toBe(false);
    expect(daemonStore.token).toBeNull();
  });
});
