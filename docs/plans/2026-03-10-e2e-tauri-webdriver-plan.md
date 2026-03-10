# E2E Tests via Tauri WebDriver — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Set up WebdriverIO + tauri-driver e2e test infrastructure and write a test that merges a Codex session's branch via the UI, verifying a PR is created on GitHub.

**Architecture:** WebdriverIO connects to tauri-driver (WebDriver server wrapping the native webview). Tests pre-seed project + session data pointing at a cloned sandbox repo, launch the built app, interact via keyboard/DOM, and verify outcomes via `gh` CLI. Cleanup removes test PRs, branches, and seeded data.

**Tech Stack:** WebdriverIO v9, tauri-driver, TypeScript, Mocha (wdio default), `gh` CLI

---

### Task 1: Install WebdriverIO Dependencies

**Files:**
- Modify: `package.json`
- Modify: `tsconfig.json` (add e2e include path)

**Step 1: Install wdio packages**

Run:
```bash
npm install --save-dev @wdio/cli @wdio/local-runner @wdio/mocha-framework @wdio/spec-reporter webdriverio
```

**Step 2: Add e2e script to package.json**

Add to the `"scripts"` section:
```json
"test:e2e": "wdio run wdio.conf.ts"
```

**Step 3: Update tsconfig.json to include e2e files**

Change the `include` array from:
```json
"include": ["src/**/*.ts", "src/**/*.svelte", "vitest-setup.ts"]
```
to:
```json
"include": ["src/**/*.ts", "src/**/*.svelte", "vitest-setup.ts", "e2e/**/*.ts"]
```

**Step 4: Commit**

```bash
git add package.json package-lock.json tsconfig.json
git commit -m "chore: add WebdriverIO dependencies for e2e tests"
```

---

### Task 2: Create WebdriverIO Config

**Files:**
- Create: `wdio.conf.ts`

**Step 1: Write the wdio config**

```typescript
import type { Options } from "@wdio/types";
import { spawn, type ChildProcess } from "node:child_process";
import path from "node:path";

let tauriDriver: ChildProcess;

export const config: Options.Testrunner = {
  autoCompileOpts: {
    tsNodeOpts: { project: "./tsconfig.json" },
  },
  specs: ["./e2e/specs/**/*.spec.ts"],
  maxInstances: 1,
  capabilities: [
    {
      // @ts-expect-error — tauri-specific capability
      "tauri:options": {
        application: path.resolve(
          "./src-tauri/target/release/bundle/macos/the-controller.app/Contents/MacOS/the-controller"
        ),
      },
    },
  ],
  framework: "mocha",
  mochaOpts: {
    ui: "bdd",
    timeout: 300_000, // 5 minutes — Codex sessions and merges are slow
  },
  reporters: ["spec"],
  onPrepare() {
    tauriDriver = spawn("tauri-driver", [], {
      stdio: [null, process.stdout, process.stderr],
    });
  },
  onComplete() {
    tauriDriver?.kill();
  },
};
```

**Step 2: Commit**

```bash
git add wdio.conf.ts
git commit -m "chore: add WebdriverIO config for tauri-driver"
```

---

### Task 3: Create Repo Setup Helper

**Files:**
- Create: `e2e/helpers/repo-setup.ts`

This helper clones the sandbox repo, creates a feature branch with a commit, and pushes it. It also handles cleanup (close PR, delete remote branch, remove temp dir).

**Step 1: Write the helper**

```typescript
import { execSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

const SANDBOX_REPO = "noel/e2e-test-sandbox";

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
  execSync(`gh repo clone ${SANDBOX_REPO} "${tempDir}" -- --depth=1`, {
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
```

**Step 2: Commit**

```bash
git add e2e/helpers/repo-setup.ts
git commit -m "chore: add e2e repo setup/cleanup helper"
```

---

### Task 4: Create Project Seeder Helper

**Files:**
- Create: `e2e/helpers/project-seed.ts`

The app reads projects from `~/.the-controller/projects/<uuid>/project.json`. This helper writes a project+session pointing at the cloned repo's worktree branch. On cleanup it removes the seeded project.

**Key context:**
- `Storage` reads from `~/.the-controller/projects/` — see `src-tauri/src/storage.rs:139`
- `Project` model — see `src-tauri/src/models.rs`
- `SessionConfig` must have `worktree_path` and `worktree_branch` for merge to work — see `src-tauri/src/commands.rs:1444-1463`

**Step 1: Write the helper**

```typescript
import { mkdirSync, writeFileSync, rmSync, existsSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";
import { randomUUID } from "node:crypto";

const CONTROLLER_DIR = path.join(homedir(), ".the-controller");
const PROJECTS_DIR = path.join(CONTROLLER_DIR, "projects");

export interface SeededProject {
  projectId: string;
  sessionId: string;
  projectDir: string;
}

/**
 * Write a project.json into ~/.the-controller/projects/<uuid>/ so the app
 * picks it up on launch. The project points at `repoPath` and has one session
 * on `branchName`.
 */
export function seedProject(
  repoPath: string,
  branchName: string
): SeededProject {
  const projectId = randomUUID();
  const sessionId = randomUUID();
  const projectDir = path.join(PROJECTS_DIR, projectId);

  mkdirSync(projectDir, { recursive: true });

  const project = {
    id: projectId,
    name: `e2e-test-${Date.now()}`,
    repo_path: repoPath,
    created_at: new Date().toISOString(),
    archived: false,
    maintainer: { enabled: false, interval_minutes: 30 },
    auto_worker: { enabled: false },
    prompts: [],
    staged_session: null,
    sessions: [
      {
        id: sessionId,
        label: branchName,
        worktree_path: repoPath, // In our test, the clone IS the worktree
        worktree_branch: branchName,
        archived: false,
        kind: "codex",
        github_issue: null,
        initial_prompt: null,
        done_commits: [],
        auto_worker_session: false,
      },
    ],
  };

  writeFileSync(
    path.join(projectDir, "project.json"),
    JSON.stringify(project, null, 2)
  );

  return { projectId, sessionId, projectDir };
}

/**
 * Remove the seeded project directory.
 */
export function cleanupSeededProject(seeded: SeededProject): void {
  if (existsSync(seeded.projectDir)) {
    rmSync(seeded.projectDir, { recursive: true, force: true });
  }
}
```

**Step 2: Commit**

```bash
git add e2e/helpers/project-seed.ts
git commit -m "chore: add e2e project seeder helper"
```

---

### Task 5: Write Smoke Test (App Launches)

**Files:**
- Create: `e2e/specs/smoke.spec.ts`

Before the merge test, verify the basic WebDriver setup works: the app launches and renders.

**Step 1: Write the smoke test**

```typescript
describe("smoke", () => {
  it("should launch and render the app", async () => {
    // The app should have a title
    const title = await browser.getTitle();
    expect(title).toBe("The Controller");
  });

  it("should render the sidebar", async () => {
    // The sidebar has the project tree
    const sidebar = await $(".sidebar");
    await sidebar.waitForExist({ timeout: 10_000 });
    expect(await sidebar.isDisplayed()).toBe(true);
  });
});
```

**Step 2: Build the app**

Run:
```bash
npm run tauri build
```

This is required because WebDriver runs against the compiled binary (configured in `wdio.conf.ts` capabilities).

**Step 3: Run the smoke test**

Run:
```bash
npm run test:e2e -- --spec e2e/specs/smoke.spec.ts
```

Expected: Both tests pass. If they fail, debug the tauri-driver / wdio connection.

**Step 4: Commit**

```bash
git add e2e/specs/smoke.spec.ts
git commit -m "test: add e2e smoke test (app launches and renders)"
```

---

### Task 6: Write Merge Workflow Test

**Files:**
- Create: `e2e/specs/merge-codex.spec.ts`

This is the main test. It verifies the full merge flow: pre-seeded Codex session → press `m` → confirm → PR created on GitHub.

**Key context:**
- `m` key triggers "finish-branch" action — see `src/lib/commands.ts:76`
- ConfirmModal appears with title "Confirm Merge" — see `src/lib/Sidebar.svelte:854`
- ConfirmModal accepts `y` key to confirm — see `src/lib/ConfirmModal.svelte:20-23`
- For Codex sessions, finish-branch writes `$the-controller-finishing-a-development-branch` to PTY — see `src/lib/Sidebar.svelte:860-867`
- The merge_session_branch backend command does rebase + push + `gh pr create` — see `src-tauri/src/commands.rs:1434-1529`
- Toast shows "PR created: {url}" on success — see `src/lib/Sidebar.svelte:598`

**Note:** The "finish-branch" flow writes a prompt to the PTY for the agent to execute. For a happy-path test without an active Codex session, we should use the `merge-session` action instead (hotkey action that calls `merge_session_branch` directly). Let me check...

Actually, looking at the code more carefully:
- `finish-branch` (key `m`): Writes a skill prompt to the PTY. Requires a live Codex session.
- `merge-session`: Calls `invoke("merge_session_branch", ...)` directly. This is the backend merge.

For the e2e test, we have two options:
1. Use `merge-session` hotkey action (direct backend merge, no PTY needed)
2. Start a real Codex session and use `finish-branch`

Option 1 is more reliable for the initial test. The merge flow is the same (rebase + push + PR). We can add a full Codex session test later.

Looking at the Sidebar code, `merge-session` is triggered via hotkey action — let me find its keybinding.

Actually, `merge-session` is dispatched from the Sidebar's hotkey handling (line 211-221 of Sidebar.svelte), but it requires `mergeSessionTarget` to be set. Let me look at how the user triggers it:

```svelte
case "merge-session": {
  const sess = ...
  mergeSessionTarget = { sessionId, projectId, label };
  break;
}
```

And this shows a ConfirmModal with "Merge Session Branch" title. Let me check the keybinding...

Looking at `commands.ts`, I don't see `merge-session` in the command list — it's only `finish-branch` with key `m`. So `merge-session` is dispatched programmatically, not from a keyboard shortcut.

This means the e2e test needs to either:
1. Use `finish-branch` (`m` key) — but this writes to PTY, needs active session
2. Execute JavaScript via WebDriver to dispatch the action directly

For a true e2e test, option 1 is better. But we need a running session.

Actually, re-reading the finish-branch flow in Sidebar.svelte:
```svelte
{#if finishBranchTarget}
  <ConfirmModal
    title="Confirm Merge"
    message="Merge this session's branch?"
    confirmLabel="Merge"
    onConfirm={() => {
      const { sessionId, kind } = finishBranchTarget;
      const isCodex = kind === "codex";
      const prompt = isCodex
        ? `$the-controller-finishing-a-development-branch`
        : `/the-controller-finishing-a-development-branch`;
      invoke("write_to_pty", { sessionId, data: prompt }).then(() => {
        invoke("write_to_pty", { sessionId, data: "\r" });
      });
    }}
  />
{/if}
```

This writes the finish-branch skill prompt to the PTY. It doesn't call `merge_session_branch`. The actual merge is done by the AGENT (Claude/Codex) executing the skill, not by the app directly.

So for a true e2e test of "merging a project which was using codex," the flow is:
1. Session with Codex is running
2. User presses `m`, confirms
3. The app writes the skill prompt to the Codex PTY
4. Codex reads the skill and executes the merge (checkout, rebase, push, create PR)
5. The PR appears

This means the test DOES need a running Codex session. The test verifies that the app correctly dispatches the merge command to Codex and Codex produces a PR.

**Step 1: Write the test**

```typescript
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
```

**Step 2: Run the test**

Run:
```bash
npm run test:e2e -- --spec e2e/specs/merge-codex.spec.ts
```

Expected: Test passes — a PR is created on the sandbox repo.

**Step 3: Commit**

```bash
git add e2e/specs/merge-codex.spec.ts
git commit -m "test: add e2e merge-codex workflow test"
```

---

### Task 7: Update .gitignore

**Files:**
- Modify: `.gitignore`

**Step 1: Add wdio logs/artifacts to gitignore**

Append to `.gitignore`:
```
# e2e test artifacts
wdio-logs/
e2e/screenshots/
```

**Step 2: Commit**

```bash
git add .gitignore
git commit -m "chore: gitignore e2e test artifacts"
```

---

## Prerequisites (Manual, One-Time)

Before running e2e tests for the first time:

1. **Install tauri-driver:**
   ```bash
   cargo install tauri-driver
   ```

2. **Create sandbox repo on GitHub:**
   ```bash
   gh repo create noel/e2e-test-sandbox --public --clone
   cd e2e-test-sandbox
   echo "# E2E Test Sandbox" > README.md
   git add README.md && git commit -m "init" && git push
   ```

3. **Build the app:**
   ```bash
   npm run tauri build
   ```

4. **Ensure `gh` CLI is authenticated:**
   ```bash
   gh auth status
   ```

5. **Ensure Codex CLI is installed:**
   ```bash
   which codex
   ```
