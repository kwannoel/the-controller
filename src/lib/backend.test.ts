import { describe, it, expect, vi, beforeEach } from "vitest";
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
