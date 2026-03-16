import { test, expect } from "@playwright/test";

test("issues hub: Enter opens find view with j/k navigation (not search focus)", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

  // Focus a project in the sidebar by pressing j (navigate down)
  await page.keyboard.press("j");
  await page.waitForTimeout(200);

  // Open issues modal with 'i' key (requires focused project)
  await page.keyboard.press("i");
  await expect(page.locator(".issues-modal")).toBeVisible({ timeout: 3_000 });

  // We should be in the hub view — verify hub menu is visible
  await expect(page.locator(".hub-menu")).toBeVisible({ timeout: 2_000 });

  // Press Enter to open find view
  await page.keyboard.press("Enter");

  // Find view should be visible (search input exists)
  const searchInput = page.locator(".issues-modal input.input");
  await expect(searchInput).toBeVisible({ timeout: 10_000 });

  // CORE ASSERTION: search input should NOT be focused — overlay should have focus for j/k nav
  await expect(searchInput).not.toBeFocused();

  // Verify the overlay (dialog container) has focus instead
  const overlay = page.locator('.overlay[role="dialog"]');
  await expect(overlay).toBeFocused();
});
