# Keystroke Visualizer Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Show app-level hotkeys as floating pills when toggled on with Cmd+K.

**Architecture:** A store holds toggle state and a keystroke queue. HotkeyManager pushes keystrokes to the store. A new component renders them as horizontally-stacked pills that fade out. Each key is its own pill — no combining sequences.

**Tech Stack:** Svelte 5 (runes), CSS animations, existing store patterns.

---

### Task 1: Keystroke visualizer store

**Files:**
- Create: `src/lib/keystroke-visualizer.ts`
- Test: `src/lib/keystroke-visualizer.test.ts`

**Step 1: Write the failing test**

```ts
// src/lib/keystroke-visualizer.test.ts
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { get } from "svelte/store";
import { keystrokeVisualizerEnabled, keystrokes, toggleKeystrokeVisualizer, pushKeystroke } from "./keystroke-visualizer";

describe("keystroke-visualizer", () => {
  beforeEach(() => {
    keystrokeVisualizerEnabled.set(false);
    keystrokes.set([]);
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.restoreAllTimers();
  });

  it("toggles enabled state", () => {
    expect(get(keystrokeVisualizerEnabled)).toBe(false);
    toggleKeystrokeVisualizer();
    expect(get(keystrokeVisualizerEnabled)).toBe(true);
    toggleKeystrokeVisualizer();
    expect(get(keystrokeVisualizerEnabled)).toBe(false);
  });

  it("pushKeystroke adds a keystroke when enabled", () => {
    keystrokeVisualizerEnabled.set(true);
    pushKeystroke("j");
    const list = get(keystrokes);
    expect(list).toHaveLength(1);
    expect(list[0].label).toBe("j");
  });

  it("pushKeystroke is a no-op when disabled", () => {
    pushKeystroke("j");
    expect(get(keystrokes)).toHaveLength(0);
  });

  it("auto-removes keystroke after 2 seconds", () => {
    keystrokeVisualizerEnabled.set(true);
    pushKeystroke("k");
    expect(get(keystrokes)).toHaveLength(1);
    vi.advanceTimersByTime(2000);
    expect(get(keystrokes)).toHaveLength(0);
  });

  it("keeps max 5 keystrokes", () => {
    keystrokeVisualizerEnabled.set(true);
    for (let i = 0; i < 7; i++) {
      pushKeystroke(String(i));
    }
    expect(get(keystrokes)).toHaveLength(5);
    // oldest are dropped
    expect(get(keystrokes)[0].label).toBe("2");
  });

  it("clears keystrokes when toggled off", () => {
    keystrokeVisualizerEnabled.set(true);
    pushKeystroke("j");
    expect(get(keystrokes)).toHaveLength(1);
    toggleKeystrokeVisualizer(); // toggles off
    expect(get(keystrokes)).toHaveLength(0);
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/keystroke-visualizer.test.ts`
Expected: FAIL — module not found

**Step 3: Write minimal implementation**

```ts
// src/lib/keystroke-visualizer.ts
import { writable, get } from "svelte/store";

interface Keystroke {
  id: number;
  label: string;
}

export const keystrokeVisualizerEnabled = writable<boolean>(false);
export const keystrokes = writable<Keystroke[]>([]);

let counter = 0;
const MAX_VISIBLE = 5;
const FADE_MS = 2000;

export function toggleKeystrokeVisualizer() {
  keystrokeVisualizerEnabled.update((v) => {
    if (v) keystrokes.set([]);
    return !v;
  });
}

export function pushKeystroke(label: string) {
  if (!get(keystrokeVisualizerEnabled)) return;
  const id = counter++;
  keystrokes.update((list) => {
    const next = [...list, { id, label }];
    return next.length > MAX_VISIBLE ? next.slice(next.length - MAX_VISIBLE) : next;
  });
  setTimeout(() => {
    keystrokes.update((list) => list.filter((k) => k.id !== id));
  }, FADE_MS);
}
```

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/keystroke-visualizer.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add src/lib/keystroke-visualizer.ts src/lib/keystroke-visualizer.test.ts
git commit -m "feat: add keystroke visualizer store"
```

---

### Task 2: KeystrokeVisualizer component

**Files:**
- Create: `src/lib/KeystrokeVisualizer.svelte`

**Step 1: Create the component**

```svelte
<!-- src/lib/KeystrokeVisualizer.svelte -->
<script lang="ts">
  import { fromStore } from "svelte/store";
  import { keystrokeVisualizerEnabled, keystrokes } from "./keystroke-visualizer";

  const enabledState = fromStore(keystrokeVisualizerEnabled);
  const keystrokesState = fromStore(keystrokes);
  let enabled = $derived(enabledState.current);
  let list = $derived(keystrokesState.current);
</script>

{#if enabled && list.length > 0}
  <div class="keystroke-container">
    {#each list as ks (ks.id)}
      <span class="keystroke-pill">{ks.label}</span>
    {/each}
  </div>
{/if}

<style>
  .keystroke-container {
    position: fixed;
    bottom: 16px;
    left: 16px;
    z-index: 1000;
    display: flex;
    flex-direction: row;
    gap: 6px;
    pointer-events: none;
  }

  .keystroke-pill {
    background: rgba(30, 30, 46, 0.85);
    color: #cdd6f4;
    border: 1px solid #313244;
    border-radius: 6px;
    padding: 4px 10px;
    font-size: 13px;
    font-family: "JetBrains Mono", "Fira Code", monospace;
    animation: pill-fade 2s ease-out forwards;
  }

  @keyframes pill-fade {
    0% { opacity: 1; }
    70% { opacity: 1; }
    100% { opacity: 0; }
  }
</style>
```

**Step 2: Commit**

```bash
git add src/lib/KeystrokeVisualizer.svelte
git commit -m "feat: add KeystrokeVisualizer component"
```

---

### Task 3: Wire into HotkeyManager and App

**Files:**
- Modify: `src/lib/HotkeyManager.svelte` — add Cmd+K toggle + push keystrokes
- Modify: `src/App.svelte` — render `KeystrokeVisualizer`

**Step 1: Add Cmd+K handler and keystroke push to HotkeyManager**

In `src/lib/HotkeyManager.svelte`, add import:

```ts
import { toggleKeystrokeVisualizer, pushKeystroke } from "./keystroke-visualizer";
```

In `onKeydown`, add Cmd+K handling right after the Cmd+S/D block (before jump mode):

```ts
// Cmd+K: toggle keystroke visualizer
if (e.metaKey && e.key === "k") {
  e.stopPropagation();
  e.preventDefault();
  toggleKeystrokeVisualizer();
  return;
}
```

At the end of `onKeydown` (after all handling, before the function closes), add a push call. The push should fire for any key that was actually handled as a hotkey (not passed through to terminal). Add at these locations:

1. After the Cmd+S/D dispatch: `pushKeystroke("⌘" + e.key.toUpperCase());`
2. After jump mode key handling: `pushKeystroke(e.key);`
3. After `handleHotkey` succeeds (returns true): `pushKeystroke(e.key);`
4. After Escape handling in ambient mode (when it does something): `pushKeystroke("Esc");`

**Step 2: Add KeystrokeVisualizer to App.svelte**

Import:
```ts
import KeystrokeVisualizer from "./lib/KeystrokeVisualizer.svelte";
```

Add `<KeystrokeVisualizer />` right before `<Toast />` (line 272).

**Step 3: Run tests**

Run: `npx vitest run`
Expected: All tests pass

**Step 4: Manual test**

Run: `npm run tauri dev`
- Press Cmd+K — visualizer toggles on (no visible change until you press keys)
- Press `j`, `k`, `s` — pills appear bottom-left, stacking horizontally, fading after 2s
- Press Cmd+K again — visualizer toggles off, pills disappear

**Step 5: Commit**

```bash
git add src/lib/HotkeyManager.svelte src/App.svelte
git commit -m "feat: wire keystroke visualizer into hotkey system"
```

---

### Task 4: Add Cmd+K to help panel

**Files:**
- Modify: `src/lib/HotkeyHelp.svelte` — add Cmd+K entry

**Step 1: Add entry to the help panel**

Find the appropriate section and add a row for `⌘K` → `Toggle keystroke visualizer`.

**Step 2: Commit**

```bash
git add src/lib/HotkeyHelp.svelte
git commit -m "feat: add Cmd+K to help panel"
```
