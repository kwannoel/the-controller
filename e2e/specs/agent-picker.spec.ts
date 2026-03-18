import { test, expect } from "@playwright/test";

const API = "http://localhost:3001";

test.describe("agent picker workflow", () => {
  test("list_agents API endpoint responds for a valid project", async ({
    request,
  }) => {
    // First, get a project to use
    const projectsRes = await request.post(`${API}/api/list_projects`, {
      data: {},
    });
    expect(projectsRes.ok()).toBe(true);
    const inventory = await projectsRes.json();
    const projects = inventory.projects ?? [];

    if (projects.length === 0) {
      test.skip();
      return;
    }

    const projectId = projects[0].id;

    // Call list_agents — this is the endpoint the user reported as broken
    const agentsRes = await request.post(`${API}/api/list_agents`, {
      data: { projectId },
    });
    expect(agentsRes.ok()).toBe(true);
    const agents = await agentsRes.json();
    expect(Array.isArray(agents)).toBe(true);
  });

  test("pressing a opens agent picker modal without errors", async ({
    page,
  }) => {
    await page.goto("/");

    // Wait for the app to load — sidebar means we're past onboarding
    const sidebar = page.locator(".sidebar");
    const onboarding = page.locator(
      ".onboarding, [data-testid='onboarding']",
    );
    await expect(sidebar.or(onboarding)).toBeVisible({ timeout: 15_000 });

    // Skip if we're in onboarding (no projects configured)
    const sidebarVisible = await sidebar.isVisible();
    if (!sidebarVisible) {
      test.skip();
      return;
    }

    // Wait for projects to load in the sidebar
    const projectItem = page.locator(".project-item");
    await expect(projectItem.first()).toBeVisible({ timeout: 10_000 });

    // Press 'a' to open the agent picker
    await page.keyboard.press("a");

    // The agent picker modal should appear
    const picker = page.locator(".picker");
    await expect(picker).toBeVisible({ timeout: 5_000 });

    // The picker title should say "Spawn Agent"
    await expect(page.locator(".picker-title")).toHaveText("Spawn Agent");

    // CORE ASSERTION: no error state should be visible
    const errorMsg = page.locator(".state-msg.error");
    const hasError = await errorMsg.isVisible().catch(() => false);
    if (hasError) {
      const errorText = await errorMsg.textContent();
      // Fail with the actual error message so we can see what went wrong
      expect(errorText).toBe(null); // This will fail and show the error
    }

    // Should show either agents or "No agents found" — NOT an error
    const agentList = page.locator(".agent-list");
    const emptyMsg = page.locator(".state-msg:not(.error)");
    await expect(agentList.or(emptyMsg)).toBeVisible({ timeout: 10_000 });
  });
});
