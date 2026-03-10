import { testRepo, seededProject } from "../../wdio.conf.js";
import { SANDBOX_REPO } from "../helpers/repo-setup.js";
import { execSync } from "node:child_process";

describe("merge codex session branch", () => {
  it("should create a PR when triggering finish-branch on a codex session", async () => {
    if (!testRepo || !seededProject) {
      throw new Error("Test repo or seeded project not initialized — check wdio.conf.ts onPrepare");
    }

    // Wait for app to load and show our seeded project
    await browser.pause(5_000);

    // Click on the session in the sidebar to focus it
    const sessionEl = await $(
      `//*[contains(@class, 'session-label') and contains(text(), '${testRepo.branchName}')]`
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
    // We poll GitHub for the PR instead of waiting for a UI toast,
    // since the toast might be transient.
    let prUrl = "";
    const maxWaitMs = 180_000; // 3 minutes
    const pollIntervalMs = 5_000;
    const startTime = Date.now();

    while (Date.now() - startTime < maxWaitMs) {
      try {
        prUrl = execSync(
          `gh pr view ${testRepo.branchName} --repo ${SANDBOX_REPO} --json url -q .url`,
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
