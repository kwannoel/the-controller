import { describe, it, expect, vi, beforeEach } from "vitest";

// Opt out of the global $lib/backend mock so we can test the real implementation.
vi.unmock("$lib/backend");

describe("backend adapter", () => {
  beforeEach(() => {
    vi.resetModules();
  });

  it("should use fetch when command is called", async () => {
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
});
