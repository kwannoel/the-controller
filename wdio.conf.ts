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
