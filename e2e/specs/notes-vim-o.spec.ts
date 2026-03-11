import { test, expect } from "@playwright/test";

test("o key creates new line and enters insert mode", async ({ page }) => {
  // Reset note content
  await page.request.post("http://localhost:3001/api/write_note", {
    data: { projectName: "fa-agent-v3", filename: "test-note.md", content: "# Test Note" },
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

  // Press o — should create new line below and enter insert mode
  await page.keyboard.press("o");
  await page.waitForTimeout(300);

  // Type text on the new line
  await page.keyboard.type("hello from o");
  await page.waitForTimeout(300);

  const content = await getContent();
  console.log("Content after o + typing:", JSON.stringify(content));

  // Verify: line 1 is original content, line 2 has the typed text
  expect(content).toContain("# Test Note");
  expect(content).toContain("hello from o");

  // Escape back to normal mode, press o again
  await page.keyboard.press("Escape");
  await page.waitForTimeout(200);
  await page.keyboard.press("o");
  await page.waitForTimeout(300);
  await page.keyboard.type("second line");
  await page.waitForTimeout(300);

  const finalContent = await getContent();
  console.log("Final content:", JSON.stringify(finalContent));

  expect(finalContent).toContain("hello from o");
  expect(finalContent).toContain("second line");
});

test("o entry key from sidebar opens editor in insert mode with new line", async ({ page }) => {
  // Reset note content
  await page.request.post("http://localhost:3001/api/write_note", {
    data: { projectName: "fa-agent-v3", filename: "test-note.md", content: "# Test Note" },
  });

  await page.goto("/");
  await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

  // Navigate to notes mode
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

  // Press o on the focused note (entry key) — should open editor with vim o
  await page.keyboard.press("o");
  await page.waitForTimeout(1000);

  const editor = page.locator('[data-testid="note-code-editor"]');
  await expect(editor).toBeVisible({ timeout: 3_000 });

  // Type text — if o entry key worked, we should be in insert mode on a new line
  await page.keyboard.type("via entry key");
  await page.waitForTimeout(300);

  const content = await page.evaluate(() => {
    const lines = document.querySelectorAll('[data-testid="note-code-editor"] .cm-line');
    return Array.from(lines).map(l => l.textContent).join('\n');
  });
  console.log("Content after entry-key o:", JSON.stringify(content));

  expect(content).toContain("# Test Note");
  expect(content).toContain("via entry key");
});
