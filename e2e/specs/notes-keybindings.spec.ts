import { test, expect } from "@playwright/test";

test("n key in notes mode opens New Folder modal", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

  // Switch to notes mode: Space then n
  await page.keyboard.press("Space");
  await page.waitForTimeout(300);
  await page.keyboard.press("n");
  await page.waitForTimeout(500);

  // Now in notes mode — press n to create a new folder
  await page.keyboard.press("n");

  // The New Folder modal should appear with "New Folder" header
  const modal = page.locator(".modal");
  await expect(modal).toBeVisible({ timeout: 3_000 });
  await expect(modal.locator(".modal-header")).toHaveText("New Folder");
});

test("c key in notes mode opens New Note modal", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

  // Switch to notes mode: Space then n
  await page.keyboard.press("Space");
  await page.waitForTimeout(300);
  await page.keyboard.press("n");
  await page.waitForTimeout(500);

  // Now in notes mode — press c to create a new note
  await page.keyboard.press("c");

  // The New Note modal should appear with "New Note" header
  const modal = page.locator(".modal");
  await expect(modal).toBeVisible({ timeout: 3_000 });
  await expect(modal.locator(".modal-header")).toHaveText("New Note");
});
