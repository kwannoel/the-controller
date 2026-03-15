---
name: the-controller-e2e-eval
description: Use when validating UI changes end-to-end before claiming work is complete — spawns a fresh server pair from the worktree and runs Playwright tests against it
---

# E2E Eval — Self-Validating UI Changes

## When to Use

You've made a UI change in a session's worktree and want to verify it actually works in a browser — not just "it compiles" but "the user can see and interact with it correctly."

## Prerequisites

The session must be staged (user pressed `v` in development mode). If not staged, ask the user to stage it first.

## Steps

### 1. Find the worktree path

Read project.json files to find the staged session's worktree:

```bash
cat ~/.the-controller/projects/*/project.json | jq -r '
  select(.staged_session != null) |
  .staged_session.session_id as $sid |
  .sessions[] | select(.id == $sid) | .worktree_path // empty
' | grep .
```

If this returns nothing, no session is staged — ask the user to press `v`.

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

- **Forgetting to stage first:** The eval needs a worktree path. Stage the session with `v`.
- **Testing against wrong servers:** Always use `eval.sh` — never manually start servers, as port conflicts with the main dev setup will cause false results.
- **Flaky waits:** Use `await expect(...).toBeVisible({ timeout: 10_000 })` instead of `waitForTimeout()` for assertions.
