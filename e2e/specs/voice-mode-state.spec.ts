import { test, expect } from "@playwright/test";

// User story:
// Actor:   A user in voice mode
// Action:  Navigates away (Space → d) and back (Space → v)
// Outcome: The label shows the current voice state (not the generic "voice mode")

test("voice mode label reflects state after re-entry", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

  // Enter voice mode: Space → v
  await page.keyboard.press("Space");
  await page.waitForTimeout(300);
  await page.keyboard.press("v");

  // The voice mode view should mount and show a label
  const label = page.locator(".voice-mode .label");
  await expect(label).toBeVisible({ timeout: 5_000 });

  // Wait for the label to update from the initial "voice mode" to an actual state
  // (pipeline emits paused/listening on success, or error on failure)
  await expect(label).not.toHaveText("voice mode", { timeout: 10_000 });
  const firstVisitLabel = await label.textContent();

  // Navigate away to development mode: Space → d
  await page.keyboard.press("Space");
  await page.waitForTimeout(300);
  await page.keyboard.press("d");

  // Sidebar should be visible again (voice mode hides it)
  await expect(page.locator(".sidebar")).toBeVisible({ timeout: 5_000 });

  // Navigate back to voice mode: Space → v
  await page.keyboard.press("Space");
  await page.waitForTimeout(300);
  await page.keyboard.press("v");

  // Label should be visible again
  await expect(label).toBeVisible({ timeout: 5_000 });

  // CORE ASSERTION: the label must NOT be the generic "voice mode" —
  // it should show the actual pipeline state (paused, listening, or error)
  await expect(label).not.toHaveText("voice mode", { timeout: 10_000 });
});
