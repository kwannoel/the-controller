import { test, expect } from "@playwright/test";

async function navigateToNote(page: any, projectName: string, noteFilename: string) {
  // Set up test note
  await page.request.post("http://localhost:3001/api/write_note", {
    data: {
      projectName,
      filename: noteFilename,
      content: "# AI Chat Test\n\nThis is the first paragraph with some text to select.\n\nThis is the second paragraph for context.",
    },
  });

  await page.goto("/");
  await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

  // Switch to notes mode
  await page.keyboard.press("Space");
  await page.waitForTimeout(300);
  await page.keyboard.press("n");
  await page.waitForTimeout(500);

  // Click the project to expand it
  const projectEl = page.locator(".sidebar").getByText(projectName, { exact: true });
  await expect(projectEl).toBeVisible({ timeout: 3_000 });
  await projectEl.click();
  await page.waitForTimeout(300);

  // Expand the project (click the arrow or press l)
  await page.keyboard.press("l");
  await page.waitForTimeout(500);

  // Click the note
  const displayName = noteFilename.replace(/\.md$/, "");
  const noteEl = page.locator(".sidebar").getByText(displayName);
  await expect(noteEl).toBeVisible({ timeout: 3_000 });
  await noteEl.click();
  await page.waitForTimeout(300);

  // Open editor with Enter
  await page.keyboard.press("Enter");
  await page.waitForTimeout(1000);

  const editor = page.locator('[data-testid="note-code-editor"]');
  await expect(editor).toBeVisible({ timeout: 5_000 });
  await expect(editor.locator(".cm-focused")).toBeVisible({ timeout: 2_000 });

  return editor;
}

test("ga in visual mode opens AI chat panel near selection", async ({ page }) => {
  const editor = await navigateToNote(page, "the-controller", "ai-chat-test.md");

  // Navigate down to the text content (past the heading and blank line)
  await page.keyboard.press("j");
  await page.waitForTimeout(100);
  await page.keyboard.press("j");
  await page.waitForTimeout(100);

  // Enter visual mode and select some text
  await page.keyboard.press("v");
  await page.waitForTimeout(200);

  // Select to end of line
  await page.keyboard.press("$");
  await page.waitForTimeout(200);

  // Take screenshot before ga
  await page.screenshot({ path: "e2e/results/before-ga.png" });

  // Press ga to trigger AI chat
  await page.keyboard.press("g");
  await page.waitForTimeout(100);
  await page.keyboard.press("a");
  await page.waitForTimeout(500);

  // Take screenshot after ga
  await page.screenshot({ path: "e2e/results/after-ga.png" });

  // Verify the AI panel appears
  const panel = page.locator('[data-testid="note-ai-panel"]');
  await expect(panel).toBeVisible({ timeout: 3_000 });

  // Verify the input is present and focused
  const input = page.locator('[data-testid="note-ai-input"]');
  await expect(input).toBeVisible();
  await expect(input).toBeFocused();

  // Verify selected text preview is shown in the panel
  const panelText = await panel.textContent();
  console.log("Panel text:", panelText);

  // Take screenshot with panel open
  await page.screenshot({ path: "e2e/results/ai-panel-open.png" });

  // Test typing in the input
  await input.fill("explain this text");
  await expect(input).toHaveValue("explain this text");

  // Take screenshot with text entered
  await page.screenshot({ path: "e2e/results/ai-panel-with-prompt.png" });

  // Test Escape dismisses the panel
  await page.keyboard.press("Escape");
  await page.waitForTimeout(300);
  await expect(panel).not.toBeVisible();

  // Take screenshot after dismiss
  await page.screenshot({ path: "e2e/results/ai-panel-dismissed.png" });
});

test("ga does nothing in normal mode (only works in visual mode)", async ({ page }) => {
  const editor = await navigateToNote(page, "the-controller", "ai-chat-test.md");

  // In normal mode (no visual selection), press ga
  await page.keyboard.press("g");
  await page.waitForTimeout(100);
  await page.keyboard.press("a");
  await page.waitForTimeout(500);

  // Panel should NOT appear
  const panel = page.locator('[data-testid="note-ai-panel"]');
  await expect(panel).not.toBeVisible();
});

test("multiline visual selection works with ga", async ({ page }) => {
  const editor = await navigateToNote(page, "the-controller", "ai-chat-test.md");

  // Navigate to content
  await page.keyboard.press("j");
  await page.waitForTimeout(100);
  await page.keyboard.press("j");
  await page.waitForTimeout(100);

  // Enter visual line mode (V) for multiline selection
  await page.keyboard.press("Shift+v");
  await page.waitForTimeout(200);

  // Select down one more line
  await page.keyboard.press("j");
  await page.waitForTimeout(200);
  await page.keyboard.press("j");
  await page.waitForTimeout(200);

  // Take screenshot of multiline selection
  await page.screenshot({ path: "e2e/results/multiline-selection.png" });

  // Press ga
  await page.keyboard.press("g");
  await page.waitForTimeout(100);
  await page.keyboard.press("a");
  await page.waitForTimeout(500);

  // Panel should appear
  const panel = page.locator('[data-testid="note-ai-panel"]');
  await expect(panel).toBeVisible({ timeout: 3_000 });

  // Selected text preview should contain multiline text
  const preview = await panel.locator("pre").textContent();
  console.log("Multiline preview:", preview);

  // Take screenshot
  await page.screenshot({ path: "e2e/results/multiline-ai-panel.png" });

  // Dismiss
  await page.keyboard.press("Escape");
  await page.waitForTimeout(300);
  await expect(panel).not.toBeVisible();
});
