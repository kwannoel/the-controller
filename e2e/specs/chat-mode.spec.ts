import { test, expect } from "@playwright/test";
import { existsSync } from "node:fs";
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
// NOTE: The daemon-reachable leg is skipped by default. It may run only when
// the daemon binaries exist and the caller opts into a shared harness with
// TCD_E2E_DAEMON_REACHABLE=1 and TCD_STATE_DIR=<shared-dir>. Playwright starts
// the Controller before this spec runs, so the Controller and daemon must
// inherit the same state dir for `/api/daemon/...` to target the same UDS.

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
  // the daemon cannot be reached. This test does not start the daemon, so
  // bootstrap fails deterministically through the same-origin gateway.
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
// Daemon-reachable leg — SKIPPED unless an explicit shared harness is present.
// ---------------------------------------------------------------------------
const daemonBinariesPresent = existsSync(DAEMON_BIN) && existsSync(FAKE_AGENT_BIN);
const daemonReachableHarnessEnabled = process.env.TCD_E2E_DAEMON_REACHABLE === "1";
const sharedStateDir = process.env.TCD_STATE_DIR;
const daemonReachableEnabled =
  daemonBinariesPresent && daemonReachableHarnessEnabled && Boolean(sharedStateDir);

test.describe("chat mode with daemon reachable", () => {
  test.skip(
    !daemonReachableEnabled,
    "daemon reachable e2e requires binaries plus " +
      "TCD_E2E_DAEMON_REACHABLE=1 and TCD_STATE_DIR shared with the Controller. " +
      "Task 21 owns the full daemon harness.",
  );

  let daemon: ChildProcess | null = null;

  test.beforeAll(async () => {
    daemon = spawn(DAEMON_BIN, [], {
      env: {
        ...process.env,
        TCD_STATE_DIR: sharedStateDir,
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
