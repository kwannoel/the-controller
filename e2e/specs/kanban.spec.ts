import { test, expect } from "@playwright/test";

/**
 * User story: A user opens the Kanban workspace mode and sees all five
 * lifecycle columns ready for issues to be placed in them.
 *
 * Actor:   User with the app loaded
 * Action:  Presses Space then 'k' to switch to the Kanban workspace
 * Outcome: Board renders with Backlog / To Do / In Progress / In Review / Done
 *
 * Scope: covers the workspace-mode wiring and column scaffold. Drag-and-drop
 * against live GitHub labels + reload-persistence would require an issue
 * fixture pipeline; those are deferred behind the same `gh`-auth gate used by
 * other GitHub-touching specs.
 */
test("kanban mode renders all five columns", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

  // Focus a project so the board has somewhere to load issues from.
  await page.keyboard.press("j");
  await page.waitForTimeout(100);

  // Enter workspace-mode picker and select kanban.
  await page.keyboard.press("Space");
  await expect(page.locator(".picker")).toBeVisible({ timeout: 3_000 });
  await page.keyboard.press("k");

  const board = page.locator('[data-testid="kanban-board"]');
  await expect(board).toBeVisible({ timeout: 5_000 });

  const expected = ["backlog", "todo", "in-progress", "in-review", "done"];
  for (const col of expected) {
    await expect(
      page.locator(`[data-testid="kanban-column-${col}"]`),
    ).toBeVisible();
  }

  // Switch back to chat mode so we don't leave state for other specs.
  await page.keyboard.press("Space");
  await page.keyboard.press("c");
});
