import { test, expect } from "@playwright/test";

/**
 * Tests the architecture mode log streaming feature.
 * When generation is triggered, the UI should show progress feedback
 * rather than a blank screen.
 */

test.describe("Architecture mode log streaming", () => {
  async function switchToArchitectureMode(page: import("@playwright/test").Page) {
    await page.goto("/");
    await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

    // Open workspace mode picker
    await page.evaluate(() => {
      window.dispatchEvent(
        new KeyboardEvent("keydown", { key: " ", code: "Space" })
      );
    });

    const picker = page.locator(".picker");
    await expect(picker).toBeVisible({ timeout: 2_000 });

    // Press 'r' for architecture
    await page.keyboard.press("r");

    await expect(page.locator(".architecture-explorer")).toBeVisible({
      timeout: 3_000,
    });
  }

  test("shows empty state with generate hint before generation", async ({
    page,
  }) => {
    await switchToArchitectureMode(page);

    // Empty state should show
    const emptyState = page.locator(".diagram-surface .empty-state");
    await expect(emptyState).toBeVisible();
    await expect(emptyState).toContainText("No architecture generated yet");
    await expect(emptyState.locator("kbd")).toHaveText("r");
  });

  test("generate button triggers generation and shows error feedback", async ({
    page,
  }) => {
    await switchToArchitectureMode(page);

    const generateBtn = page.locator(".generate-action");
    await expect(generateBtn).toBeVisible();
    await expect(generateBtn).toHaveText("Generate");
    await expect(generateBtn).toBeEnabled();

    // Click generate — codex is not available in e2e so this will error,
    // but the key assertion is that we get visible error feedback rather
    // than silently failing
    await generateBtn.click();

    // An error message should appear (server returns not-implemented or
    // codex fails)
    const errorMsg = page.locator(".generation-error");
    await expect(errorMsg).toBeVisible({ timeout: 15_000 });

    // Button should re-enable after the error
    await expect(generateBtn).toBeEnabled({ timeout: 5_000 });
    await expect(generateBtn).toHaveText("Generate");
  });

  test("architecture explorer has two-pane layout", async ({ page }) => {
    await switchToArchitectureMode(page);

    await expect(page.locator(".diagram-pane")).toBeVisible();
    await expect(page.locator(".inspector-rail")).toBeVisible();

    // Inspector should have components section and details section
    await expect(
      page.locator(".component-list-pane .section-title")
    ).toHaveText("Components");
    await expect(
      page.locator(".component-list-pane .section-count")
    ).toHaveText("0");

    // Placeholder text when no architecture is generated
    await expect(page.locator(".placeholder-copy")).toHaveText(
      "Generate architecture to see components."
    );
  });
});
