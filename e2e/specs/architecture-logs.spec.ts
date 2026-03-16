import { test, expect } from "@playwright/test";

/**
 * Tests the architecture mode log streaming feature.
 * When generation is triggered, the UI should stream real-time progress
 * logs showing evidence collection, prompt building, etc.
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

    const emptyState = page.locator(".diagram-surface .empty-state");
    await expect(emptyState).toBeVisible();
    await expect(emptyState).toContainText("No architecture generated yet");
    await expect(emptyState.locator("kbd")).toHaveText("r");
  });

  test("streams log lines during architecture generation", async ({
    page,
  }) => {
    test.setTimeout(180_000); // codex exec has 120s timeout

    await switchToArchitectureMode(page);

    const generateBtn = page.locator(".generate-action");
    await expect(generateBtn).toBeVisible();
    await expect(generateBtn).toHaveText("Generate");

    // Click generate
    await generateBtn.click();

    // Button should become disabled with "Generate…"
    await expect(generateBtn).toBeDisabled({ timeout: 2_000 });

    // The log output container should appear with streaming lines.
    // Evidence collection is pure Rust (no external deps) so we'll
    // see scanning and evidence logs before codex even starts.
    const logOutput = page.locator(".log-output");
    await expect(logOutput).toBeVisible({ timeout: 30_000 });

    // Verify meaningful progress content
    const logLines = logOutput.locator(".log-line");
    await expect(logLines.first()).toBeVisible({ timeout: 5_000 });

    // Should contain the evidence scanning phase
    await expect(logOutput).toContainText("Scanning repository for evidence", {
      timeout: 5_000,
    });

    // Should list collected evidence
    await expect(logOutput).toContainText("evidence files", {
      timeout: 10_000,
    });

    // Wait for generation to complete (success or failure)
    await expect(generateBtn).toBeEnabled({ timeout: 150_000 });

    // Collect final log text for the test output
    if (await logOutput.isVisible().catch(() => false)) {
      const allText = await logOutput.innerText();
      console.log("Architecture generation logs:\n" + allText);
    }

    // Check final state: either a rendered diagram or an error
    const hasResult = await page
      .locator(".diagram-render")
      .isVisible()
      .catch(() => false);
    const hasError = await page
      .locator(".generation-error")
      .isVisible()
      .catch(() => false);
    expect(hasResult || hasError).toBe(true);
  });

  test("architecture explorer has two-pane layout", async ({ page }) => {
    await switchToArchitectureMode(page);

    await expect(page.locator(".diagram-pane")).toBeVisible();
    await expect(page.locator(".inspector-rail")).toBeVisible();

    await expect(
      page.locator(".component-list-pane .section-title")
    ).toHaveText("Components");
    await expect(
      page.locator(".component-list-pane .section-count")
    ).toHaveText("0");

    await expect(page.locator(".placeholder-copy")).toHaveText(
      "Generate architecture to see components."
    );
  });
});
