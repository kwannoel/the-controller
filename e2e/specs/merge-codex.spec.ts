import { test, expect } from "@playwright/test";
import { setupTestRepo, cleanupTestRepo, SANDBOX_REPO, type TestRepo } from "../helpers/repo-setup";
import { seedProject, cleanupSeededProject, type SeededProject } from "../helpers/project-seed";
import { execSync } from "node:child_process";

let repo: TestRepo;
let seeded: SeededProject;

test.beforeAll(() => {
  repo = setupTestRepo();
  seeded = seedProject(repo.localPath, repo.branchName);
});

test.afterAll(() => {
  if (repo) cleanupTestRepo(repo);
  if (seeded) cleanupSeededProject(seeded);
});

test("merge codex session branch creates a PR", async ({ page }) => {
  await page.goto("/");

  // Wait for sidebar to render with our seeded project
  const sessionEl = page.locator(`.session-label`, { hasText: repo.branchName });
  await expect(sessionEl).toBeVisible({ timeout: 15_000 });

  // Click to focus the session
  await sessionEl.click();

  // Press 'm' to trigger finish-branch
  await page.keyboard.press("m");

  // ConfirmModal should appear
  await expect(page.locator(".modal-header", { hasText: "Confirm Merge" })).toBeVisible({
    timeout: 5_000,
  });

  // Press 'y' to confirm
  await page.keyboard.press("y");

  // Poll GitHub for the PR
  let prUrl = "";
  const maxWaitMs = 180_000;
  const pollIntervalMs = 5_000;
  const startTime = Date.now();

  while (Date.now() - startTime < maxWaitMs) {
    try {
      prUrl = execSync(
        `gh pr view ${repo.branchName} --repo ${SANDBOX_REPO} --json url -q .url`,
        { encoding: "utf-8" },
      ).trim();
      if (prUrl) break;
    } catch {
      // Not yet
    }
    await page.waitForTimeout(pollIntervalMs);
  }

  expect(prUrl).toMatch(/github\.com/);
});
