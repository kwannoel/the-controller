import { test, expect } from "@playwright/test";

test("u key in normal mode undoes the last change", async ({ page }) => {
  // Reset note content to a known state
  await page.request.post("/api/write_note", {
    data: { projectName: "fa-agent-v3", filename: "test-note.md", content: "# Undo Test" },
  });

  await page.goto("/");
  await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

  // Navigate to notes mode and open editor
  await page.keyboard.press("Space");
  await page.waitForTimeout(300);
  await page.keyboard.press("n");
  await page.waitForTimeout(500);
  await page.keyboard.press("j");
  await page.waitForTimeout(200);
  await page.keyboard.press("l");
  await page.waitForTimeout(300);
  await page.keyboard.press("j");
  await page.waitForTimeout(200);
  await page.keyboard.press("Enter");
  await page.waitForTimeout(1000);

  const editor = page.locator('[data-testid="note-code-editor"]');
  await expect(editor).toBeVisible({ timeout: 3_000 });
  await expect(editor.locator(".cm-focused")).toBeVisible({ timeout: 2_000 });

  const getContent = () => page.evaluate(() => {
    const lines = document.querySelectorAll('[data-testid="note-code-editor"] .cm-line');
    return Array.from(lines).map(l => l.textContent).join('\n');
  });

  // Enter insert mode with 'o', type some text
  await page.keyboard.press("o");
  await page.waitForTimeout(300);
  await page.keyboard.type("text to undo");
  await page.waitForTimeout(300);

  const contentBeforeUndo = await getContent();
  console.log("Before undo:", JSON.stringify(contentBeforeUndo));
  expect(contentBeforeUndo).toContain("text to undo");

  // Escape to normal mode, then press u to undo
  await page.keyboard.press("Escape");
  await page.waitForTimeout(300);
  await page.keyboard.press("u");
  await page.waitForTimeout(500);

  // CORE ASSERTION: the typed text should be gone after undo
  const contentAfterUndo = await getContent();
  console.log("After undo:", JSON.stringify(contentAfterUndo));
  expect(contentAfterUndo).not.toContain("text to undo");
  expect(contentAfterUndo).toContain("# Undo Test");
});
