import { test, expect } from "@playwright/test";

/**
 * User Story:
 * Actor:   A user in notes mode with a note open in the editor
 * Action:  Presses Cmd+L to toggle the agent chat sidebar
 * Outcome: The agent chat sidebar appears on the right side
 */

async function openNoteInEditor(page: any) {
  const projectName = "the-controller";
  const noteFilename = "cmd-l-test.md";

  // Create test note
  await page.request.post("http://localhost:3001/api/write_note", {
    data: {
      projectName,
      filename: noteFilename,
      content: "# Cmd+L Test\n\nSome test content for sidebar toggle.",
    },
  });

  await page.goto("/");
  await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

  // Switch to notes mode: Space then n
  await page.keyboard.press("Space");
  await page.waitForTimeout(300);
  await page.keyboard.press("n");
  await page.waitForTimeout(500);

  // Click the project to select it
  const projectEl = page
    .locator(".sidebar")
    .getByText(projectName, { exact: true });
  await expect(projectEl).toBeVisible({ timeout: 3_000 });
  await projectEl.click();
  await page.waitForTimeout(300);

  // Expand the project
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

test("Cmd+L in notes mode opens agent chat sidebar", async ({ page }) => {
  await openNoteInEditor(page);

  // Sidebar should not be visible initially
  const sidebar = page.locator(".notes-chat-sidebar");
  await expect(sidebar).not.toBeVisible();

  // Press Cmd+L to open the sidebar
  await page.keyboard.press("Meta+l");

  // The agent chat sidebar should appear
  await expect(sidebar).toBeVisible({ timeout: 5_000 });

  // Take screenshot for visual verification
  await page.screenshot({ path: "e2e/results/cmd-l-sidebar-open.png" });
});

test("Cmd+L toggles sidebar closed when already open", async ({ page }) => {
  await openNoteInEditor(page);

  const sidebar = page.locator(".notes-chat-sidebar");

  // Open sidebar
  await page.keyboard.press("Meta+l");
  await expect(sidebar).toBeVisible({ timeout: 5_000 });

  // Press Cmd+L again to close
  await page.keyboard.press("Meta+l");
  await expect(sidebar).not.toBeVisible({ timeout: 3_000 });
});
