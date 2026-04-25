import { test, expect } from "@playwright/test";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawn, type ChildProcess } from "node:child_process";

// User story:
// Actor:   A user wanting to use the new "chat" workspace mode backed by the
//          the-controller-daemon.
// Action:  Enters chat mode (Space -> c) with no daemon running; then (when
//          the daemon is built) starts the daemon against the fake_agent and
//          retries the connection.
// Outcome: Empty state is shown when daemon is unreachable. After the daemon
//          is started, retrying transitions out of the empty state.
//
// NOTE: The daemon-reachable leg of this test is skipped by default because
// it requires the daemon & fake_agent binaries to be built at the canonical
// paths below. The axum server (server/src/main.rs) exposes
// `/api/read_daemon_token` so the browser harness can load the daemon token
// when the daemon is running.

const DAEMON_REPO = "/Users/noelkwan/projects/the-controller-daemon";
const DAEMON_BIN = `${DAEMON_REPO}/target/release/the-controller-daemon`;
const FAKE_AGENT_BIN = `${DAEMON_REPO}/target/debug/fake_agent`;

async function switchToChatMode(page: import("@playwright/test").Page) {
  await page.keyboard.press("Space");
  // Wait for the workspace-mode picker to mount before issuing the next key.
  // Using the picker's class-based locator matches the pattern in
  // architecture-logs.spec.ts and avoids races on slow CI.
  await expect(page.locator(".picker")).toBeVisible({ timeout: 3_000 });
  await page.keyboard.press("c");
}

test("chat mode shows DaemonEmptyState when daemon is unreachable", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

  await switchToChatMode(page);

  // Core assertion: the DaemonEmptyState component renders its heading when
  // the daemon cannot be reached. The Axum server exposes `read_daemon_token`,
  // but this test does not start the daemon, so bootstrap fails deterministically.
  await expect(page.getByRole("heading", { name: "Daemon not running" })).toBeVisible({
    timeout: 5_000,
  });

  // The Retry button should be present and clickable without error.
  const retry = page.getByRole("button", { name: "Retry" });
  await expect(retry).toBeVisible();
  await retry.click();
  // Still unreachable after retry (same browser-mode constraint), so the
  // empty state must remain visible. This is a regression guard: if a
  // refactor silently hides the empty state, this fails.
  await expect(page.getByRole("heading", { name: "Daemon not running" })).toBeVisible({
    timeout: 2_000,
  });
});

// ---------------------------------------------------------------------------
// Daemon-reachable leg — SKIPPED unless the daemon binaries are built.
// ---------------------------------------------------------------------------
const daemonBinariesPresent = existsSync(DAEMON_BIN) && existsSync(FAKE_AGENT_BIN);

test.describe("chat mode with daemon reachable", () => {
  test.skip(
    !daemonBinariesPresent,
    `daemon binaries not built at ${DAEMON_BIN} / ${FAKE_AGENT_BIN}. ` +
      "Run scripts/chat-integration-daemon.sh build to produce them.",
  );

  let daemon: ChildProcess | null = null;
  let stateDir: string | null = null;

  test.beforeAll(async () => {
    stateDir = mkdtempSync(join(tmpdir(), "tcd-e2e-"));
    daemon = spawn(DAEMON_BIN, [], {
      env: {
        ...process.env,
        TCD_STATE_DIR: stateDir,
        TCD_AGENT_CLAUDE_BINARY: FAKE_AGENT_BIN,
      },
      stdio: "pipe",
    });
    // Give the daemon a moment to write its token file and start listening.
    await new Promise((r) => setTimeout(r, 1500));
  });

  test.afterAll(async () => {
    if (daemon && daemon.pid) {
      daemon.kill("SIGTERM");
      await new Promise((r) => setTimeout(r, 300));
      if (!daemon.killed) daemon.kill("SIGKILL");
    }
    if (stateDir) {
      try {
        rmSync(stateDir, { recursive: true, force: true });
      } catch {
        // best effort
      }
    }
  });

  test("workspace renders out of the empty state after Retry", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });
    await switchToChatMode(page);

    await page.getByRole("button", { name: "Retry" }).click();
    await expect(
      page.getByRole("heading", { name: "Daemon not running" }),
    ).toHaveCount(0, { timeout: 5_000 });
  });
});
