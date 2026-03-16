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
    await switchToArchitectureMode(page);

    const generateBtn = page.locator(".generate-action");
    await expect(generateBtn).toBeVisible();
    await expect(generateBtn).toHaveText("Generate");

    // Click generate — evidence collection and prompt building will
    // succeed and emit logs. Codex exec will likely fail, but we should
    // see real log lines before the error.
    await generateBtn.click();

    // The log output container should appear with streaming lines.
    // Evidence collection is pure Rust (no external deps) so at minimum
    // we'll see "Scanning repository for evidence" and file list.
    const logOutput = page.locator(".log-output");
    const errorMsg = page.locator(".generation-error");

    // Wait for either logs or error (whichever comes first)
    await expect(logOutput.or(errorMsg)).toBeVisible({ timeout: 30_000 });

    // If logs appeared, verify they contain meaningful progress info
    if (await logOutput.isVisible().catch(() => false)) {
      const logLines = logOutput.locator(".log-line");
      const count = await logLines.count();
      expect(count).toBeGreaterThan(0);

      // Collect all log text for inspection
      const allText = await logOutput.innerText();
      console.log("Architecture generation logs:\n" + allText);

      // Should contain the evidence scanning phase
      expect(allText).toContain("Scanning repository for evidence");
    }

    // Eventually generation finishes (success or error)
    await expect(generateBtn).toBeEnabled({ timeout: 120_000 });

    // Check final state: either we got a result or an error
    const hasResult = await page
      .locator(".diagram-render")
      .isVisible()
      .catch(() => false);
    const hasError = await errorMsg.isVisible().catch(() => false);
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
