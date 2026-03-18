import { test, expect } from "@playwright/test";

/**
 * User Stories for the `i` issues workflow:
 *
 * 1. User presses `i` with a project focused → issues hub modal appears
 * 2. User presses `f` in hub → find view with search focused
 * 3. User presses `c` in hub → create view with title input focused
 * 4. User presses Esc in hub → modal closes
 * 5. User navigates issues with j/k in find view
 * 6. User presses Esc in find view → returns to hub
 * 7. User presses Enter in hub → find view with j/k nav (overlay focused, not search)
 */

/** Wait for sidebar with projects loaded, focus a project via j, verify focus took. */
async function focusProject(page: import("@playwright/test").Page) {
  await expect(page.locator(".sidebar")).toBeVisible({ timeout: 15_000 });
  // Wait for at least one project to be rendered
  await expect(page.locator(".project-header").first()).toBeVisible({ timeout: 10_000 });
  await page.keyboard.press("j");
  await expect(page.locator(".focus-target")).toBeVisible({ timeout: 3_000 });
}

/** Focus a project and open the issues hub modal. */
async function openIssuesHub(page: import("@playwright/test").Page) {
  await focusProject(page);
  await page.keyboard.press("i");
  await expect(page.locator(".issues-modal .hub-menu")).toBeVisible({ timeout: 5_000 });
}

test("story 1: pressing i with focused project opens issues hub", async ({ page }) => {
  await page.goto("/");
  await focusProject(page);

  await page.keyboard.press("i");

  // CORE ASSERTION: issues hub modal appears with Create and Find options
  const modal = page.locator(".issues-modal");
  await expect(modal).toBeVisible({ timeout: 5_000 });
  await expect(modal.locator(".modal-header")).toHaveText("Issues");
  await expect(modal.locator(".hub-menu")).toBeVisible();
  await expect(modal.locator(".hub-option")).toHaveCount(2);
});

test("story 2: pressing f in hub opens find view with search focused", async ({ page }) => {
  await page.goto("/");
  await openIssuesHub(page);

  await page.keyboard.press("f");

  // CORE ASSERTION: find view opens with search input focused
  const searchInput = page.locator('.issues-modal input[placeholder="Search issues..."]');
  await expect(searchInput).toBeVisible({ timeout: 10_000 });
  await expect(searchInput).toBeFocused();
});

test("story 3: pressing c in hub opens create view with title input focused", async ({ page }) => {
  await page.goto("/");
  await openIssuesHub(page);

  await page.keyboard.press("c");

  // CORE ASSERTION: create view with title input focused
  const modal = page.locator(".issues-modal");
  await expect(modal.locator(".modal-header")).toHaveText("New Issue");
  const titleInput = modal.locator('input[placeholder="Issue title"]');
  await expect(titleInput).toBeVisible({ timeout: 3_000 });
  await expect(titleInput).toBeFocused();
});

test("story 4: pressing Esc in hub closes the modal", async ({ page }) => {
  await page.goto("/");
  await openIssuesHub(page);

  await page.keyboard.press("Escape");

  // CORE ASSERTION: modal is gone
  await expect(page.locator(".issues-modal")).not.toBeVisible({ timeout: 3_000 });
});

test("story 5: Esc in find view returns to hub (not closes modal)", async ({ page }) => {
  await page.goto("/");
  await openIssuesHub(page);

  // Enter find view
  await page.keyboard.press("f");
  await expect(page.locator('.issues-modal input[placeholder="Search issues..."]')).toBeVisible({ timeout: 10_000 });

  // Press Esc — should go back to hub, not close modal
  await page.keyboard.press("Escape");

  // CORE ASSERTION: back at hub, modal still open
  await expect(page.locator(".issues-modal")).toBeVisible();
  await expect(page.locator(".issues-modal .hub-menu")).toBeVisible({ timeout: 3_000 });
});

test("story 6: Esc in create view title stage returns to hub", async ({ page }) => {
  await page.goto("/");
  await openIssuesHub(page);

  // Enter create view
  await page.keyboard.press("c");
  await expect(page.locator('.issues-modal input[placeholder="Issue title"]')).toBeVisible({ timeout: 3_000 });

  // Press Esc — should go back to hub
  await page.keyboard.press("Escape");

  // CORE ASSERTION: back at hub
  await expect(page.locator(".issues-modal .hub-menu")).toBeVisible({ timeout: 3_000 });
});

test("story 7: Enter in hub opens find view with overlay focused (j/k nav mode)", async ({ page }) => {
  await page.goto("/");
  await openIssuesHub(page);

  // Press Enter (not f) to open find view in nav mode
  await page.keyboard.press("Enter");

  // Find view should be visible
  const searchInput = page.locator('.issues-modal input[placeholder="Search issues..."]');
  await expect(searchInput).toBeVisible({ timeout: 10_000 });

  // CORE ASSERTION: search input should NOT be focused — overlay has focus for j/k
  await expect(searchInput).not.toBeFocused();
  await expect(page.locator('.overlay[role="dialog"]')).toBeFocused();
});

test("story 8: create flow — title → priority → complexity stages", async ({ page }) => {
  await page.goto("/");
  await openIssuesHub(page);

  // Enter create view
  await page.keyboard.press("c");
  const titleInput = page.locator('.issues-modal input[placeholder="Issue title"]');
  await expect(titleInput).toBeFocused({ timeout: 3_000 });

  // Type a title and press Enter
  await titleInput.fill("Test issue title");
  await page.keyboard.press("Enter");

  // CORE ASSERTION: priority stage appears with j/k options
  const modal = page.locator(".issues-modal");
  await expect(modal.locator(".title-preview")).toHaveText("Test issue title", { timeout: 3_000 });
  await expect(modal.locator(".option-row")).toBeVisible();

  // Select low priority (j)
  await page.keyboard.press("j");

  // CORE ASSERTION: complexity stage appears with priority badge shown
  await expect(modal.locator(".selected-badge")).toBeVisible({ timeout: 3_000 });
  await expect(modal.locator(".selected-badge")).toHaveText("low priority");
  await expect(modal.locator(".option-row")).toBeVisible();
});
