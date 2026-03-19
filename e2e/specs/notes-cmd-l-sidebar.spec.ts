import { test, expect } from "@playwright/test";

/**
 * User Story:
 * Actor:   A user in notes mode with a note open in the editor
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

  // Wait for the sidebar to show folders
  const sidebar = page.locator(".sidebar");
  await expect(sidebar.locator("h2")).toHaveText("Notes", { timeout: 3_000 });

  // Click the first folder entry (the expand arrow button)
  const firstFolderToggle = sidebar.locator("button").filter({ hasText: "▶" }).first();
  await expect(firstFolderToggle).toBeVisible({ timeout: 3_000 });
  await firstFolderToggle.click();
  await page.waitForTimeout(500);

  // After expanding, there should be a note entry to click
  // The expanded folder shows note entries — click the first one
  // Look for a ▼ (expanded state) and then find the note entry below
  const noteEntries = sidebar.locator(".note-entry, .sidebar-item").filter({ hasNotText: /▶|▼/ });

  // If no specific note-entry class, try clicking the first item below the folder
  // Use keyboard: press j to move to the note, then Enter to open it
  await page.keyboard.press("j"); // move to folder
  await page.waitForTimeout(200);
  await page.keyboard.press("l"); // expand folder
  await page.waitForTimeout(500);
  await page.keyboard.press("j"); // move to note
  await page.waitForTimeout(200);
  await page.keyboard.press("Enter"); // open note
  await page.waitForTimeout(1000);

  const editor = page.locator('[data-testid="note-code-editor"]');
  await expect(editor).toBeVisible({ timeout: 5_000 });
  await expect(editor.locator(".cm-focused")).toBeVisible({ timeout: 2_000 });

  return editor;
}

test("Cmd+L in notes mode opens agent chat sidebar", async ({ page }) => {
  await openFirstNoteInEditor(page);

  // Sidebar should not be visible initially
  const chatSidebar = page.locator(".notes-chat-sidebar");
  await expect(chatSidebar).not.toBeVisible();

  // Press Cmd+L to open the sidebar
  await page.keyboard.press("Meta+l");

  // The agent chat sidebar should appear
  await expect(chatSidebar).toBeVisible({ timeout: 5_000 });

  // Take screenshot for visual verification
  await page.screenshot({ path: "e2e/results/cmd-l-sidebar-open.png" });
});

test("Cmd+L toggles sidebar closed when already open", async ({ page }) => {
  await openFirstNoteInEditor(page);

  const chatSidebar = page.locator(".notes-chat-sidebar");

  // Open sidebar
  await page.keyboard.press("Meta+l");
  await expect(chatSidebar).toBeVisible({ timeout: 5_000 });

  // Press Cmd+L again to close
  await page.keyboard.press("Meta+l");
  await expect(chatSidebar).not.toBeVisible({ timeout: 3_000 });
});
