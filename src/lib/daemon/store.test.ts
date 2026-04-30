import { describe, it, expect, vi, beforeEach } from "vitest";

describe("daemon store bootstrap", () => {
  beforeEach(() => vi.resetModules());

  it("sets reachable=true on successful same-origin ping without reading a token", async () => {
    const commandMock = vi.fn(async (cmd: string) => {
      throw new Error("unexpected command " + cmd);
    });
    vi.doMock("$lib/backend", () => ({
      command: commandMock,
      listen: () => () => {},
    }));
    const fetchMock = vi.fn(async () => ({ ok: true, status: 200, json: async () => [] } as any));
    vi.stubGlobal("fetch", fetchMock);
    const { daemonStore, bootstrap } = await import("./store.svelte");
    await bootstrap();
    expect(daemonStore.reachable).toBe(true);
    expect(commandMock).not.toHaveBeenCalled();
    expect("token" in daemonStore).toBe(false);
    const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe("/api/daemon/sessions");
    expect(init.headers).not.toMatchObject({ Authorization: expect.any(String) });
  });

  it("sets reachable=false when ping fails", async () => {
    const commandMock = vi.fn(async (cmd: string) => {
      throw new Error("unexpected command " + cmd);
    });
    vi.doMock("$lib/backend", () => ({
      command: commandMock,
      listen: () => () => {},
    }));
    vi.stubGlobal("fetch", vi.fn(async () => { throw new TypeError("connect refused"); }));
    const { daemonStore, bootstrap } = await import("./store.svelte");
    await bootstrap();
    expect(daemonStore.reachable).toBe(false);
    expect(commandMock).not.toHaveBeenCalled();
  });
});
