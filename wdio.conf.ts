import type { Options } from "@wdio/types";
import { spawn, type ChildProcess } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { setupTestRepo, cleanupTestRepo, type TestRepo } from "./e2e/helpers/repo-setup.js";
import { seedProject, cleanupSeededProject, type SeededProject } from "./e2e/helpers/project-seed.js";

const APP_BINARY = path.resolve(
  "./src-tauri/target/release/bundle/macos/the-controller.app/Contents/MacOS/the-controller"
);

let tauriDriver: ChildProcess;

// Shared state between onPrepare (setup) and onComplete (cleanup).
// Tests access these via `import { testRepo, seededProject } from "../../wdio.conf.js"`.
export let testRepo: TestRepo | undefined;
export let seededProject: SeededProject | undefined;

export const config: Options.Testrunner = {
  autoCompileOpts: {
    tsNodeOpts: { project: "./tsconfig.e2e.json" },
  },
  specs: ["./e2e/specs/**/*.spec.ts"],
  maxInstances: 1,
  capabilities: [
    {
      // @ts-expect-error — tauri-specific capability
      "tauri:options": {
        application: APP_BINARY,
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
    if (!existsSync(APP_BINARY)) {
      throw new Error(
        `App binary not found at ${APP_BINARY}. Run "npm run tauri build" first.`
      );
    }

    // Seed project data BEFORE app launches so it appears in the project list.
    testRepo = setupTestRepo();
    seededProject = seedProject(testRepo.localPath, testRepo.branchName);

    tauriDriver = spawn("tauri-driver", [], {
      stdio: [null, process.stdout, process.stderr],
    });
  },
  onComplete() {
    tauriDriver?.kill();
    if (testRepo) cleanupTestRepo(testRepo);
    if (seededProject) cleanupSeededProject(seededProject);
  },
};
