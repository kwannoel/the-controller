import { test, expect } from "@playwright/test";

/**
 * User story: A user in Architecture mode triggers generation and sees
 * real-time log feedback, then a rendered diagram with components.
 *
 * Actor:   User in Architecture mode with a project loaded
 * Action:  Presses 'r' to generate architecture
 * Outcome: Log lines stream during generation, then an SVG diagram
 *          renders with a populated components list and details panel
 */

async function switchToArchitectureMode(page: import("@playwright/test").Page) {
  await page.goto("/");
  await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

  // Click a project in the sidebar to establish focus (required for
  // hotkeys like 'r' that need a focused project)
  const firstProject = page.locator(".project-header").first();
  await expect(firstProject).toBeVisible({ timeout: 5_000 });
  await firstProject.click();

  // Switch to architecture mode via real keyboard
  await page.keyboard.press("Space");
  await expect(page.locator(".picker")).toBeVisible({ timeout: 3_000 });
  await page.keyboard.press("r");
  await expect(page.locator(".architecture-explorer")).toBeVisible({
    timeout: 5_000,
  });
}

test("architecture generation streams logs then renders diagram with components", async ({
  page,
}) => {
  test.setTimeout(180_000);

  await switchToArchitectureMode(page);

  // Precondition: empty state visible before generation
  await expect(
    page.locator(".diagram-surface .empty-state")
  ).toContainText("No architecture generated yet");

  // Action: trigger generation with real keyboard press
  await page.keyboard.press("r");

  // Outcome 1: log lines stream showing generation progress
  const logOutput = page.locator(".log-output");
  await expect(logOutput).toBeVisible({ timeout: 30_000 });
  await expect(logOutput).toContainText("Scanning repository for evidence", {
    timeout: 5_000,
  });
  await expect(logOutput).toContainText("evidence files", {
    timeout: 10_000,
  });

  // Outcome 2: after generation completes, a rendered SVG diagram appears
  const diagramSvg = page.locator(".diagram-render svg");
  await expect(diagramSvg).toBeVisible({ timeout: 150_000 });

  // Outcome 3: components list populates with at least one component
  const componentButtons = page.locator(".component-list button");
  await expect(componentButtons.first()).toBeVisible({ timeout: 5_000 });
  const componentCount = await componentButtons.count();
  expect(componentCount).toBeGreaterThan(0);

  // Outcome 4: component count badge reflects the actual number
  await expect(page.locator(".section-count")).toHaveText(
    String(componentCount)
  );

  // Outcome 5: first component is auto-selected and shows details
  await expect(componentButtons.first()).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator(".detail-name")).toBeVisible();
  await expect(page.locator(".summary")).toBeVisible();

  // Outcome 6: clicking a different component updates the details
  if (componentCount > 1) {
    const secondBtn = componentButtons.nth(1);
    const secondName = await secondBtn.innerText();
    await secondBtn.click();
    await expect(page.locator(".detail-name")).toHaveText(secondName);
  }

  // Outcome 7: generate button shows "Regenerate" after success
  await expect(page.locator(".generate-action")).toHaveText("Regenerate");
});
