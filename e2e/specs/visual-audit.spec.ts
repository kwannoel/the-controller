import { test, expect } from "@playwright/test";
import path from "node:path";

const SCREENSHOT_DIR = path.resolve("e2e/results/visual-audit");

test.describe("Visual audit — screenshots of every workspace mode", () => {
  test("capture all modes", async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.goto("/");
    await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

    // 1. Development mode (default)
    await page.waitForTimeout(500);
    await page.screenshot({ path: path.join(SCREENSHOT_DIR, "01-development.png"), fullPage: false });

    // 2. Open help modal
    await page.locator(".btn-help").click();
    await expect(page.locator(".modal")).toBeVisible({ timeout: 3_000 });
    await page.screenshot({ path: path.join(SCREENSHOT_DIR, "02-help-modal.png"), fullPage: false });
    await page.keyboard.press("Escape");
    await expect(page.locator(".modal")).not.toBeVisible();

    // 3. Open workspace mode picker
    await page.keyboard.press("Space");
    const picker = page.locator(".picker");
    if (await picker.isVisible({ timeout: 2_000 }).catch(() => false)) {
      await page.screenshot({ path: path.join(SCREENSHOT_DIR, "03-workspace-picker.png"), fullPage: false });

      // 4. Switch to Agents
      await page.keyboard.press("a");
      await expect(page.locator(".sidebar-header h2")).toHaveText("Agents", { timeout: 3_000 });
      await page.waitForTimeout(300);
      await page.screenshot({ path: path.join(SCREENSHOT_DIR, "04-agents.png"), fullPage: false });

      // 5. Switch to Architecture
      await page.keyboard.press("Space");
      if (await picker.isVisible({ timeout: 2_000 }).catch(() => false)) {
        await page.keyboard.press("r");
        await page.waitForTimeout(300);
        await page.screenshot({ path: path.join(SCREENSHOT_DIR, "05-architecture.png"), fullPage: false });
      }

      // 6. Switch to Notes
      await page.keyboard.press("Space");
      if (await picker.isVisible({ timeout: 2_000 }).catch(() => false)) {
        await page.keyboard.press("n");
        await expect(page.locator(".sidebar-header h2")).toHaveText("Notes", { timeout: 3_000 });
        await page.waitForTimeout(300);
        await page.screenshot({ path: path.join(SCREENSHOT_DIR, "06-notes.png"), fullPage: false });
      }

      // 7. Switch to Infrastructure
      await page.keyboard.press("Space");
      if (await picker.isVisible({ timeout: 2_000 }).catch(() => false)) {
        await page.keyboard.press("i");
        await page.waitForTimeout(300);
        await page.screenshot({ path: path.join(SCREENSHOT_DIR, "07-infrastructure.png"), fullPage: false });
      }

      // 8. Switch to Voice (no sidebar)
      await page.keyboard.press("Space");
      if (await picker.isVisible({ timeout: 2_000 }).catch(() => false)) {
        await page.keyboard.press("v");
        await page.waitForTimeout(500);
        await page.screenshot({ path: path.join(SCREENSHOT_DIR, "08-voice.png"), fullPage: false });
      }

      // 9. Switch back to Development
      await page.keyboard.press("Space");
      if (await picker.isVisible({ timeout: 2_000 }).catch(() => false)) {
        await page.keyboard.press("d");
        await page.waitForTimeout(300);
        await page.screenshot({ path: path.join(SCREENSHOT_DIR, "09-development-returned.png"), fullPage: false });
      }
    }
  });
});
