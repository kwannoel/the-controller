---
name: the-controller-e2e-eval
description: Use when validating UI changes end-to-end before claiming work is complete — spawns a fresh server pair from the worktree and runs Playwright tests against it
---

# E2E Eval — Self-Validating UI Changes

## When to Use

You've made a UI change in a session's worktree and want to verify it actually works in a browser — not just "it compiles" but "the user can see and interact with it correctly."

## Steps

### 1. Commit your changes

`eval.sh` requires a clean worktree. Commit all your work before proceeding.

### 2. Find your worktree path

```bash
BRANCH=$(git rev-parse --abbrev-ref HEAD)
WORKTREE_PATH=$(cat ~/.the-controller/projects/*/project.json | jq -r --arg b "$BRANCH" '
  .sessions[] | select(.worktree_branch == $b) | .worktree_path
' | head -1)
```

### 3. Write a targeted Playwright test

Create a spec file in `e2e/specs/` following existing patterns (see `e2e/specs/smoke.spec.ts` for the simplest example):

```typescript
import { test, expect } from "@playwright/test";

test("description of what the UI change does", async ({ page }) => {
  await page.goto("/");
  // Setup: navigate to the right state
  // Action: perform the user interaction
  // Assert: verify the expected outcome
});
```

Use helpers from `e2e/helpers/` if you need seeded projects or test repos.

### 4. Run the targeted test

Run from the repo root (the directory containing `e2e/eval.sh` and `playwright.config.ts`):

```bash
./e2e/eval.sh "$WORKTREE_PATH" e2e/specs/<your-test>.spec.ts
```

The script handles staging (clean check, rebase onto main, npm install), starts Axum + Vite, runs Playwright, and tears down.

### 5. Run the regression suite

```bash
./e2e/eval.sh "$WORKTREE_PATH"
```

This runs ALL specs to catch regressions.

### 6. Interpret results

- **All pass:** Commit the new test to the worktree. It becomes part of the regression suite.
- **Targeted test fails:** Your UI change has a bug. Fix the code, re-run.
- **Regression test fails:** Your change broke something else. Investigate.
- **Deeper investigation needed:** Use the-controller-debugging-ui-with-playwright skill for the 4-phase root cause analysis.

## Common Mistakes

- **Uncommitted changes:** `eval.sh` fails fast if the worktree is dirty. Always commit before running.
- **Rebase conflicts:** If your branch conflicts with main, eval.sh aborts the rebase and exits. Resolve conflicts manually, then re-run.
- **Testing against wrong servers:** Always use `eval.sh` — never manually start servers, as port conflicts with the main dev setup will cause false results.
- **Flaky waits:** Use `await expect(...).toBeVisible({ timeout: 10_000 })` instead of `waitForTimeout()` for assertions.
