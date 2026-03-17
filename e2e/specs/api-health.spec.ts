import { test, expect } from "@playwright/test";

const API = "http://localhost:3001";

test.describe("backend API health", () => {
  test("list_projects endpoint responds", async ({ request }) => {
    const res = await request.post(`${API}/api/list_projects`, { data: {} });
    expect(res.ok()).toBe(true);
    const body = await res.json();
    expect(body).toBeDefined();
  });

  test("check_onboarding endpoint responds", async ({ request }) => {
    const res = await request.post(`${API}/api/check_onboarding`, {
      data: {},
    });
    expect(res.ok()).toBe(true);
  });

  test("home_dir returns a path string", async ({ request }) => {
    const res = await request.post(`${API}/api/home_dir`, { data: {} });
    expect(res.ok()).toBe(true);
    const body = await res.text();
    expect(body.length).toBeGreaterThan(0);
  });

  test("load_keybindings returns valid config", async ({ request }) => {
    const res = await request.post(`${API}/api/load_keybindings`, {
      data: {},
    });
    expect(res.ok()).toBe(true);
    const body = await res.json();
    expect(typeof body).toBe("object");
  });
});
