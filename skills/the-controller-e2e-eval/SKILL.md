---
name: the-controller-e2e-eval
description: Use when validating UI changes end-to-end before claiming work is complete — spawns a fresh server pair from the worktree and runs Playwright tests against it
---

# E2E Eval — Self-Validating UI Changes

## When to Use

You've made a UI change in a session's worktree and want to verify it actually works in a browser — not just "it compiles" but "the user can see and interact with it correctly."

## Steps

### 1. Stage the session

Commit all your changes first, then trigger staging via the Controller socket. Find the project and session IDs from project.json using the current branch:

```bash
BRANCH=$(git rev-parse --abbrev-ref HEAD)
read PROJECT_ID SESSION_ID WORKTREE_PATH < <(
  cat ~/.the-controller/projects/*/project.json | jq -r --arg b "$BRANCH" '
    . as $p |
    .sessions[] | select(.worktree_branch == $b) |
    "\($p.id) \(.id) \(.worktree_path)"
  ' | head -1
)
```

If a session is already staged, the response will say so. Unstage it first if needed or skip this step.

Send the stage command and wait for the response (staging includes rebase + npm install + launching dev server, which takes a few minutes):

```bash
printf 'stage:%s:%s\n' "$PROJECT_ID" "$SESSION_ID" | nc -U -w 300 /tmp/the-controller.sock
```

The response will be either `staged:<port>` on success or `error:<message>` on failure. If you get an error about uncommitted changes, commit first and retry.

The `$WORKTREE_PATH` variable from above is the path to pass to `eval.sh`.

### 2. Write a targeted Playwright test

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

### 3. Run the targeted test

Run from the repo root (the directory containing `e2e/eval.sh` and `playwright.config.ts`):

```bash
./e2e/eval.sh <worktree-path> e2e/specs/<your-test>.spec.ts
```

The script handles everything: finds free ports, starts Axum + Vite from the worktree, runs Playwright, tears down.

### 4. Run the regression suite

```bash
./e2e/eval.sh <worktree-path>
```

This runs ALL specs to catch regressions.

### 5. Interpret results

- **All pass:** Commit the new test to the worktree. It becomes part of the regression suite.
- **Targeted test fails:** Your UI change has a bug. Fix the code, re-run.
- **Regression test fails:** Your change broke something else. Investigate.
- **Deeper investigation needed:** Use the-controller-debugging-ui-with-playwright skill for the 4-phase root cause analysis.

## Common Mistakes

- **Staging with uncommitted changes:** The socket staging command fails if the worktree is dirty. Always commit before sending the `stage:` command.
- **Testing against wrong servers:** Always use `eval.sh` — never manually start servers, as port conflicts with the main dev setup will cause false results.
- **Flaky waits:** Use `await expect(...).toBeVisible({ timeout: 10_000 })` instead of `waitForTimeout()` for assertions.
