import { describe, it, expect, vi, beforeEach } from "vitest";
import { DaemonClient, DaemonHttpError } from "./client";

function mockFetch(responses: Array<{ status: number; body: unknown }>) {
  const queue = [...responses];
  return vi.fn(async () => {
    const r = queue.shift()!;
    return {
      ok: r.status >= 200 && r.status < 300,
      status: r.status,
      json: async () => r.body,
      text: async () => JSON.stringify(r.body),
    } as any;
  });
}

describe("DaemonClient", () => {
  beforeEach(() => vi.restoreAllMocks());

  it("calls same-origin /api/daemon routes without exposing auth", async () => {
    const fetchMock = mockFetch([{ status: 200, body: [] }]);
    vi.stubGlobal("fetch", fetchMock);

    const c = new DaemonClient("/api/daemon");
    await c.listSessions();

    const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe("/api/daemon/sessions");
    expect(init.headers).not.toMatchObject({ Authorization: expect.any(String) });
  });

  it("builds same-origin websocket URLs", () => {
    const c = new DaemonClient("/api/daemon");
    expect(c.wsUrl("s1", 5)).toBe("/api/daemon/sessions/s1/stream?since=5");
  });

  it("throws DaemonHttpError on 404", async () => {
    vi.stubGlobal("fetch", mockFetch([{ status: 404, body: { error: "not found" } }]));
    const c = new DaemonClient("/api/daemon");
    await expect(c.getSession("missing")).rejects.toMatchObject({
      name: "DaemonHttpError",
      status: 404,
    });
  });
});
