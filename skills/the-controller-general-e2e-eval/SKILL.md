---
name: the-controller-general-e2e-eval
description: Use when validating UI changes end-to-end in any project before claiming work is complete — runs Playwright tests against the project's dev server to prove user stories work
---

# General E2E Eval — User Story Validation

## When to Use

You've made a UI change in any web project and need to prove it works from the user's perspective — not "it compiles" or "the DOM element exists," but "the user can do the thing and see the result."

Use this skill for projects that don't have a custom eval script. If the project has `e2e/eval.sh` or similar, use the project-specific eval skill instead.

## Hard Rules

These are non-negotiable. Every test must follow all of them.

### 1. User story first, always

Before writing a single line of test code, state the user story:

- **Actor**: Who is performing the action (e.g., "a logged-in user on the settings page")
- **Action**: What they physically do (e.g., "clicks 'Save' after changing their display name")
- **Outcome**: What they expect to see (e.g., "a success toast appears and the new name is shown in the header")

The test's **final assertion** must verify the **outcome**. If the outcome doesn't happen, the test fails. No exceptions.

### 2. No silent passes

**BANNED** — the `if (visible)` escape hatch:

```typescript
// NEVER DO THIS — silently passes when the feature is broken
const visible = await foo.isVisible().catch(() => false);
if (visible) {
  await expect(foo).toHaveText("expected");
}
```

**REQUIRED** — unconditional assertion with a timeout:

```typescript
// DO THIS — fails loudly when the feature is broken
await expect(foo).toBeVisible({ timeout: 10_000 });
await expect(foo).toHaveText("expected");
```

If the element should be there, assert it. If it might legitimately not be there, that's a different test or a precondition you need to set up.

### 3. No mocking the interface

- **No `page.evaluate()` to set store state** when a real interaction exists. If the user clicks a button to open a modal, the test clicks the button.
- **No synthetic `dispatchEvent(new KeyboardEvent(...))`** — use `page.keyboard.press()`.
- **No stubbing API responses** — the test runs against a real dev server.

The only acceptable use of `page.evaluate()` is to **read** state for assertions (e.g., checking a computed style), never to **write** state that bypasses the UI.

### 4. Assert outcomes, not implementation

**WRONG** — asserting CSS values and class names:

```typescript
// This proves the DOM exists, not that the feature works
await expect(page.locator(".search-results")).toBeVisible();
await expect(page.locator(".search-input")).toHaveClass(/active/);
```

**RIGHT** — asserting what the user sees after the action:

```typescript
// This proves the feature works end-to-end
await page.locator('[placeholder="Search..."]').fill("playwright");
await page.keyboard.press("Enter");
await expect(
  page.locator('[data-testid="search-results"]').locator("article")
).toHaveCount(3, { timeout: 10_000 });
await expect(page.locator("article").first()).toContainText("playwright");
```

Structure checks (element exists, layout is correct) are fine as **preconditions** or **smoke tests**, but they are not user story validation. The final assertion must be about the **result of the user's action**.

### 5. One story per test

Each `test()` block validates exactly one user story. Don't bundle "log in AND update profile AND log out" into one test. The log-out is cleanup, not a story.

### 6. Timeouts reflect reality

If an action triggers an async operation (e.g., API call, server-side rendering, AI generation), the timeout must be long enough for it to complete in CI. Use 60s for AI/generation operations, 10s for navigation, 30s for server-dependent renders.

## Steps

### 1. Ensure Playwright is installed

Check if the project has Playwright configured:

```bash
ls playwright.config.{ts,js,mjs} 2>/dev/null
```

If missing, initialize:

```bash
npm init playwright@latest
# or: pnpm create playwright / yarn create playwright
```

### 2. Configure the dev server

Playwright can automatically start your dev server. Ensure `playwright.config.ts` has a `webServer` block:

```typescript
// playwright.config.ts
export default defineConfig({
  webServer: {
    command: "npm run dev",      // match your project's dev command
    port: 3000,                   // match your dev server's port
    reuseExistingServer: true,
  },
  // ...
});
```

If the project has **multiple servers** (e.g., a separate API backend):

```typescript
webServer: [
  {
    command: "npm run dev:api",
    port: 3001,
    reuseExistingServer: true,
    timeout: 120_000,
  },
  {
    command: "npm run dev",
    port: 3000,
    reuseExistingServer: true,
  },
],
```

### 3. Define the core user story

State the user story explicitly before writing test code:

```
Actor:   A new visitor on the signup page
Action:  Fills out the form and clicks "Create Account"
Outcome: They are redirected to the dashboard and see a welcome message with their name
```

This is the contract. The test must not pass unless this outcome is achieved.

### 4. Write the test

Create a spec file that follows the hard rules above:

```typescript
import { test, expect } from "@playwright/test";

test("signup: completing the form creates an account and shows the dashboard", async ({ page }) => {
  await page.goto("/signup");
  await expect(page.locator("h1")).toHaveText("Create Account", { timeout: 10_000 });

  // Fill the form using real interactions
  await page.locator('[name="email"]').fill("test@example.com");
  await page.locator('[name="password"]').fill("securepassword123");
  await page.locator('[name="displayName"]').fill("Test User");

  // Submit via button click, not form.submit()
  await page.locator('button[type="submit"]').click();

  // CORE ASSERTION: user lands on dashboard with their name
  await expect(page).toHaveURL(/\/dashboard/, { timeout: 10_000 });
  await expect(page.locator('[data-testid="welcome-message"]')).toContainText("Test User");
});
```

Key properties:
- Uses real interactions (`fill`, `click`), not `dispatchEvent`
- No `if (visible)` guards — every assertion is unconditional
- Final assertion is the **user-visible outcome** (dashboard + welcome message), not a DOM class
- Timeouts are realistic for the operation

### 5. Run the targeted test

```bash
npx playwright test path/to/your-test.spec.ts
```

If you have multiple Playwright projects configured, target the right one:

```bash
npx playwright test --project=e2e path/to/your-test.spec.ts
```

### 6. Run the regression suite

```bash
npx playwright test
```

### 7. Interpret results

- **Core assertion fails:** The feature doesn't work. Fix the code, not the test. Do not weaken the assertion to make it pass.
- **Precondition fails:** The test can't reach the starting state (e.g., page never loads). Check that the dev server starts correctly.
- **Regression test fails:** Your change broke something else. Investigate.
- **Server didn't start:** Check the `webServer` config in `playwright.config.ts` — wrong port, wrong command, or missing dependencies.

**CRITICAL:** If the core user story assertion fails, you must fix the implementation until it passes. Never downgrade the assertion, add an escape hatch, or mark the test as skipped. The user story is the contract.

## Common Mistakes

- **Weakening assertions to pass:** If the test fails, the feature is broken. Fix the feature. Don't weaken the test.
- **Using `waitForTimeout()`:** Use `await expect(...).toBeVisible({ timeout: N })` instead. Hard sleeps hide timing bugs.
- **Testing against wrong server:** If your project's dev server is already running, Playwright's `reuseExistingServer: true` will connect to it instead of starting a fresh one. This is usually fine, but be aware of it.
- **Missing dependencies:** If testing in a fresh checkout, run the package manager install first (`npm install`, `pnpm install`, etc.).
- **Port conflicts:** If Playwright can't start the dev server because the port is taken, either stop the existing server or use dynamic port allocation.
