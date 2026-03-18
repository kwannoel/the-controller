import { test, expect } from "@playwright/test";

test("app loads and renders main UI", async ({ page }) => {
  await page.goto("/");
  await expect(page).toHaveTitle("The Controller");
  // In CI (no onboarding config) the app shows the onboarding screen;
  // in a configured environment it shows the sidebar.
  const sidebar = page.locator(".sidebar");
  const onboarding = page.locator(".onboarding, [data-testid='onboarding']");
  await expect(sidebar.or(onboarding)).toBeVisible({ timeout: 15_000 });
});
