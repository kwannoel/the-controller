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

  it("listSessions sets Authorization header and returns array", async () => {
    const fetchMock = mockFetch([{ status: 200, body: [{ id: "s1", label: "x", agent: "claude" }] }]);
    vi.stubGlobal("fetch", fetchMock);
    const c = new DaemonClient("http://127.0.0.1:4867", "TOK");
    const sessions = await c.listSessions();
    expect(sessions[0].id).toBe("s1");
    const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe("http://127.0.0.1:4867/sessions");
    expect((init.headers as Record<string, string>).Authorization).toBe("Bearer TOK");
  });

  it("throws DaemonHttpError on 404", async () => {
    vi.stubGlobal("fetch", mockFetch([{ status: 404, body: { error: "not found" } }]));
    const c = new DaemonClient("http://127.0.0.1:4867", "TOK");
    await expect(c.getSession("missing")).rejects.toMatchObject({
      name: "DaemonHttpError",
      status: 404,
    });
  });
});
