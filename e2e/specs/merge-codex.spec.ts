import {
  setupTestRepo,
  cleanupTestRepo,
  type TestRepo,
} from "../helpers/repo-setup.js";
import {
  seedProject,
  cleanupSeededProject,
  type SeededProject,
} from "../helpers/project-seed.js";
import { execSync } from "node:child_process";

const SANDBOX_REPO = "noel/e2e-test-sandbox";

describe("merge codex session branch", () => {
  let repo: TestRepo;
  let seeded: SeededProject;

  before(async () => {
    // 1. Clone sandbox repo, create branch with commit, push
    repo = setupTestRepo();

    // 2. Seed project data so the app loads it
    seeded = seedProject(repo.localPath, repo.branchName);

    // 3. Wait for app to load and show our project
    await browser.pause(5_000); // Give app time to load projects from disk
  });

  after(async () => {
    cleanupTestRepo(repo);
    cleanupSeededProject(seeded);
  });

  it("should create a PR when triggering finish-branch on a codex session", async () => {
    // Click on the session in the sidebar to focus it
    const sessionEl = await $(
      `//*[contains(@class, 'session-label') and contains(text(), '${repo.branchName}')]`
    );
    await sessionEl.waitForExist({ timeout: 15_000 });
    await sessionEl.click();

    // Press 'm' to trigger finish-branch
    await browser.keys("m");

    // ConfirmModal should appear with "Confirm Merge"
    const modal = await $(".modal-header=Confirm Merge");
    await modal.waitForExist({ timeout: 5_000 });

    // Press 'y' to confirm
    await browser.keys("y");

    // Wait for the skill to be written to PTY and Codex to execute the merge.
    // This can take a while — Codex needs to read the skill, run git commands,
    // push, and create a PR.
    // We poll GitHub for the PR instead of waiting for a UI toast,
    // since the toast might be transient.
    let prUrl = "";
    const maxWaitMs = 180_000; // 3 minutes
    const pollIntervalMs = 5_000;
    const startTime = Date.now();

    while (Date.now() - startTime < maxWaitMs) {
      try {
        prUrl = execSync(
          `gh pr view ${repo.branchName} --repo ${SANDBOX_REPO} --json url -q .url`,
          { encoding: "utf-8" }
        ).trim();
        if (prUrl) break;
      } catch {
        // PR doesn't exist yet, keep polling
      }
      await browser.pause(pollIntervalMs);
    }

    expect(prUrl).toMatch(/github\.com/);
  });
});
