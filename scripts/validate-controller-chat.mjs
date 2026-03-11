#!/usr/bin/env node

import { spawnSync } from "node:child_process";

function run(command, args) {
  const result = spawnSync(command, args, {
    stdio: "inherit",
    encoding: "utf8",
  });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

run("codex", ["--version"]);
run("cargo", ["--version"]);
run("cargo", [
  "test",
  "--manifest-path",
  "src-tauri/Cargo.toml",
  "--test",
  "controller_chat_validation",
  "--",
  "--ignored",
  "--nocapture",
]);
