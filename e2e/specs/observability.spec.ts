import { test, expect } from "@playwright/test";

// User story:
// Actor:   A developer navigating workspace modes from the keyboard.
// Action:  Opens the workspace picker and chooses Observe.
// Outcome: The agent trace workspace appears with a clear empty state.

test("observe mode opens the agent trace workspace", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

  await page.keyboard.press("Space");
  await expect(page.locator(".picker")).toBeVisible({ timeout: 3_000 });
  await page.keyboard.press("o");

  await expect(page.locator(".sidebar-header h2")).toHaveText("Observe", {
    timeout: 3_000,
  });
  await expect(page.getByText("Select an agent to inspect.")).toBeVisible({
    timeout: 10_000,
  });
});
