---
name: the-controller-debugging-ui-with-playwright
description: Use when a UI behavior doesn't work as expected (key not responding, focus lost, mode not changing) and code reading alone isn't enough — runs Playwright in browser mode to reproduce, isolate, and pinpoint the root cause
---

# Debugging UI/UX Issues with Playwright

## When to Use

A user reports a UI behavior that doesn't work as expected — a key doesn't respond, focus is lost, a mode doesn't change, a visual state is wrong — and reading the code doesn't make it obvious why.

**Don't skip to fixes.** Use the `the-controller-systematic-debugging` skill's iron law: no fixes without root cause investigation first. This skill is the browser-specific investigation technique.

## Setup Checklist

Before writing any test, verify all three servers:

```bash
# 1. Backend (port 3001)
lsof -i :3001 -P | grep LISTEN
# If not running:
cd src-tauri && cargo run --bin server --features server

# 2. Frontend dev server (port 1420)
lsof -i :1420 -P | grep LISTEN
# CRITICAL: verify it's serving the right directory
ps -p $(lsof -t -i :1420) -o command=
# If it says /Users/noel/projects/the-controller — that's the main repo
# Code changes MUST go to that directory, not a worktree

# 3. Playwright dependencies
npm install  # worktrees need their own node_modules
```

**Silent failure trap**: The axum server has a `fallback_handler` that returns `null` for any unregistered API route. If a feature silently does nothing in browser mode, check if the API endpoint exists in `src-tauri/src/bin/server.rs`.

## The Four Phases

### Phase 1: Reproduce

Write a Playwright test that does exactly what the user did. No cleverness — just automate the steps.

```typescript
test("reproduce: o key should enter insert mode", async ({ page }) => {
  await page.goto("/");
  // Navigate to the right state
  await page.keyboard.press("Space");
  await page.waitForTimeout(300);
  await page.keyboard.press("n");
  // ... exact steps the user took ...

  // Screenshot at each step
  await page.screenshot({ path: "e2e/results/step-01.png" });

  // Assert the expected outcome
  expect(content).toContain("typed text");
});
```

Run with `npx playwright test <file> --project=e2e --headed`.

If the test passes, the bug isn't what you think. If it fails, you have a reproducible case.

### Phase 2: Isolate

Break the problem into smaller pieces to narrow the scope:

- **Specificity**: If `o` doesn't work, does `i` work? Does `a` work? Does ANY command of that type work?
- **Granularity**: If typing doesn't work, does one character work? Two? Use individual `page.keyboard.press()` calls with `waitForTimeout()` between them.
- **Mechanism**: Does `page.keyboard.type()` work? Does `page.keyboard.insertText()` work? Does `page.evaluate()` to directly call APIs work?

Check content after EACH individual action:

```typescript
await page.keyboard.press("o");
await page.waitForTimeout(300);
content = await getContent();
console.log("After o:", JSON.stringify(content));

await page.keyboard.press("h");
await page.waitForTimeout(200);
content = await getContent();
console.log("After o + h:", JSON.stringify(content));
```

### Phase 3: Instrument

Once you know the symptom (e.g., "second character isn't typed"), add diagnostics:

#### Focus tracking (most common root cause)

```typescript
// Install global focus monitor
await page.evaluate(() => {
  document.addEventListener("focusin", (e) => {
    console.log("[FOCUS] focusin:", (e.target as HTMLElement)?.tagName,
      (e.target as HTMLElement)?.className?.substring(0, 60));
  });
  document.addEventListener("focusout", (e) => {
    console.log("[FOCUS] focusout:", (e.target as HTMLElement)?.tagName,
      (e.target as HTMLElement)?.className?.substring(0, 60));
  });
  // Blur listener with stack trace — this is the money shot
  const target = document.querySelector('.cm-content');
  target?.addEventListener("blur", () => {
    console.log("[FOCUS] BLUR!", new Error().stack);
  });
});

// Check activeElement after each action
const checkFocus = async (label: string) => {
  const info = await page.evaluate(() => ({
    activeTag: document.activeElement?.tagName,
    activeClass: document.activeElement?.className?.substring(0, 60),
  }));
  console.log(`[FOCUS ${label}]`, JSON.stringify(info));
};
```

#### Console log capture

```typescript
page.on("console", (msg) => {
  if (msg.text().includes("MY-DEBUG")) consoleLogs.push(msg.text());
});
```

**Remember**: add `console.log` to the file the dev server is serving, not the worktree copy.

### Phase 4: Pinpoint

Follow the stack trace from Phase 3 to the root cause.

**The stack trace from a blur event handler is the single most useful piece of evidence.** It tells you exactly which code path stole focus or destroyed the element.

Example from the `o`-key bug:
```
cm-content BLUR!
  at _EditorView.destroy (chunk-FJ5OGNHB.js:8197:23)
  at CodeMirrorNoteEditor.svelte:74:10
  at execute_effect_teardown (chunk-HXN5OQ4Q.js:3225:17)
```

This immediately revealed: Svelte's `$effect` cleanup was destroying the CodeMirror view. The effect was re-running because it tracked `value` (a reactive prop), which changed on every keystroke.

## Known Gotchas

### Svelte 5 `$effect` + imperative libraries (CodeMirror, xterm.js)

`$effect` tracks ALL reactive values read in its body. If a view-creation effect reads a prop like `value` via `buildState(value)`, every content change will:

1. Re-run the effect
2. Execute cleanup (destroying the view)
3. Recreate the view from scratch
4. Lose all internal state (vim mode, cursor position, focus)

**Fix**: Use `untrack()` for values that should only be read once:

```typescript
import { untrack } from "svelte";

$effect(() => {
  if (!hostEl || view) return;
  const initialValue = untrack(() => value);
  view = new EditorView({ state: buildState(initialValue), parent: hostEl });
  // ...
});
```

This pattern applies to ANY imperative library mounted in a Svelte 5 `$effect`. The value sync should be a separate effect.

### Notes storage path

`~/.the-controller/notes/{project_name}/`, not `~/.the-controller/projects/{id}/`.
