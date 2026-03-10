import { execSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

export const SANDBOX_REPO = "kwannoel/e2e-test-sandbox";

export interface TestRepo {
  localPath: string;
  branchName: string;
}

/**
 * Clone the sandbox repo, create a feature branch with a trivial commit, push it.
 */
export function setupTestRepo(): TestRepo {
  const tempDir = mkdtempSync(path.join(tmpdir(), "e2e-sandbox-"));
  const branchName = `e2e-test-${Date.now()}`;

  // Clone
  execSync(`gh repo clone ${SANDBOX_REPO} "${tempDir}"`, {
    stdio: "inherit",
  });

  // Create branch + commit
  execSync(
    [
      `git checkout -b ${branchName}`,
      `echo "e2e test ${Date.now()}" >> e2e-test.txt`,
      `git add e2e-test.txt`,
      `git commit -m "e2e: test commit for merge workflow"`,
      `git push -u origin ${branchName}`,
    ].join(" && "),
    { cwd: tempDir, stdio: "inherit" }
  );

  return { localPath: tempDir, branchName };
}

/**
 * Clean up: close any PR, delete remote branch, remove local clone.
 */
export function cleanupTestRepo(repo: TestRepo): void {
  try {
    // Close PR if one exists
    execSync(
      `gh pr close ${repo.branchName} --repo ${SANDBOX_REPO} --delete-branch 2>/dev/null || true`,
      { cwd: repo.localPath, stdio: "inherit" }
    );
  } catch {
    // Best effort
  }

  try {
    // Delete remote branch (in case PR close didn't delete it)
    execSync(
      `git push origin --delete ${repo.branchName} 2>/dev/null || true`,
      { cwd: repo.localPath, stdio: "inherit" }
    );
  } catch {
    // Best effort
  }

  // Remove local clone
  rmSync(repo.localPath, { recursive: true, force: true });
}
