import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e/specs",
  timeout: 300_000, // 5 minutes for slow Codex workflows
  use: {
    baseURL: process.env.BASE_URL ?? "http://localhost:1420",
    video: "retain-on-failure",
  },
  webServer: [
    {
      command: "cd src-tauri && cargo run --bin server --features server",
      port: 3001,
      reuseExistingServer: true,
      timeout: 120_000,
    },
    {
      command: "npm run dev",
      port: 1420,
      reuseExistingServer: true,
    },
  ],
  projects: [
    {
      name: "e2e",
      use: { browserName: "chromium" },
    },
    {
      name: "demo",
      use: {
        browserName: "chromium",
        video: "on",
        viewport: { width: 1280, height: 800 },
      },
    },
  ],
  outputDir: "e2e/results",
});
