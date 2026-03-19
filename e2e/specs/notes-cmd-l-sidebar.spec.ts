import { test, expect } from "@playwright/test";

/**
 * User Story:
 * Actor:   A user in notes mode with a note open and focus in the editor
 * Action:  Presses Cmd+L to toggle the agent chat sidebar
 * Outcome: The agent chat sidebar appears on the right side
 */

async function openFirstNoteInEditor(page: any) {
  await page.goto("/");
  await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

  // Switch to notes mode: Space then n
  await page.keyboard.press("Space");
  await page.waitForTimeout(300);
  await page.keyboard.press("n");
  await page.waitForTimeout(500);

  const sidebar = page.locator(".sidebar");
  await expect(sidebar.locator("h2")).toHaveText("Notes", { timeout: 3_000 });

  // Expand the first folder
  const folderToggle = sidebar.locator("button.btn-expand").first();
  await expect(folderToggle).toBeVisible({ timeout: 3_000 });
  await folderToggle.click();
  await page.waitForTimeout(500);

  // Double-click the first note-item to open it
  const noteItem = sidebar.locator(".note-item").first();
  await expect(noteItem).toBeVisible({ timeout: 3_000 });
  await noteItem.dblclick();
  await page.waitForTimeout(1000);

  const editor = page.locator('[data-testid="note-code-editor"]');
  await expect(editor).toBeVisible({ timeout: 5_000 });

  return editor;
}

test("Cmd+L opens sidebar when focus is in notes editor", async ({ page }) => {
  const editor = await openFirstNoteInEditor(page);

  // Click into the CodeMirror content area to ensure focus is inside the editor
  const cmContent = editor.locator(".cm-content");
  await expect(cmContent).toBeVisible({ timeout: 3_000 });
  await cmContent.click();
  await page.waitForTimeout(300);

  // Verify the editor is focused
  await expect(editor.locator(".cm-focused")).toBeVisible({ timeout: 2_000 });

  // Sidebar should not be visible initially
  const chatSidebar = page.locator(".notes-chat-sidebar");
  await expect(chatSidebar).not.toBeVisible();

  // Press Cmd+L to open the sidebar
  await page.keyboard.press("Meta+l");

  // The agent chat sidebar should appear
  await expect(chatSidebar).toBeVisible({ timeout: 5_000 });

  await page.screenshot({ path: "e2e/results/cmd-l-sidebar-open.png" });
});

test("Cmd+L toggles sidebar closed when already open", async ({ page }) => {
  const editor = await openFirstNoteInEditor(page);

  // Focus inside the editor
  const cmContent = editor.locator(".cm-content");
  await cmContent.click();
  await page.waitForTimeout(300);

  const chatSidebar = page.locator(".notes-chat-sidebar");

  // Open sidebar
  await page.keyboard.press("Meta+l");
  await expect(chatSidebar).toBeVisible({ timeout: 5_000 });

  // Press Cmd+L again to close
  await page.keyboard.press("Meta+l");
  await expect(chatSidebar).not.toBeVisible({ timeout: 3_000 });
});
