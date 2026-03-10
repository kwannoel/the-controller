import { test, expect } from "@playwright/test";

test("app loads and renders sidebar", async ({ page }) => {
  await page.goto("/");
  await expect(page).toHaveTitle("The Controller");
  await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });
});
