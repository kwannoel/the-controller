import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("backend adapter", () => {
  beforeEach(() => {
    vi.resetModules();
    delete (window as any).__TAURI__;
  });

  it("should use invoke when __TAURI__ is present", async () => {
    (window as any).__TAURI__ = {};
    const { command } = await import("./backend");
    const { invoke } = await import("@tauri-apps/api/core");
    (invoke as any).mockResolvedValue({ id: "123" });

    const result = await command("list_projects");
    expect(invoke).toHaveBeenCalledWith("list_projects", undefined);
    expect(result).toEqual({ id: "123" });
  });

  it("should use fetch when __TAURI__ is absent", async () => {
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
