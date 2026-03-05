# Image Paste Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enable image pasting in the terminal so Claude Code can receive images from the clipboard.

**Architecture:** Intercept Cmd+V/Ctrl+V in the custom key handler, check clipboard for images via Tauri's clipboard plugin, and send a bracket paste sequence to the PTY to trigger Claude Code's image paste handling.

**Tech Stack:** Svelte 5, TypeScript, xterm.js, @tauri-apps/plugin-clipboard-manager, Tauri v2

---

### Task 1: Add clipboard image read permission

**Files:**
- Modify: `src-tauri/capabilities/default.json`

**Step 1: Add the permission**

Add `"clipboard-manager:allow-read-image"` to the permissions array in `src-tauri/capabilities/default.json`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Capability for the main window",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "opener:default",
    "clipboard-manager:allow-read-text",
    "clipboard-manager:allow-read-image"
  ]
}
```

**Step 2: Verify Tauri builds**

Run: `cd src-tauri && cargo check 2>&1 | tail -5`
Expected: compiles without errors

**Step 3: Commit**

```bash
git add src-tauri/capabilities/default.json
git commit -m "feat: add clipboard image read permission for image paste (#30)"
```

---

### Task 2: Add clipboard image check helper

**Files:**
- Create: `src/lib/clipboard.ts`
- Create: `src/lib/clipboard.test.ts`

**Step 1: Write the failing test**

Create `src/lib/clipboard.test.ts`:

```typescript
import { describe, it, expect, vi } from "vitest";
import { clipboardHasImage } from "./clipboard";

// Mock the Tauri clipboard plugin
vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({
  readImage: vi.fn(),
}));

import { readImage } from "@tauri-apps/plugin-clipboard-manager";
const mockReadImage = vi.mocked(readImage);

describe("clipboardHasImage", () => {
  it("returns true when clipboard contains an image", async () => {
    mockReadImage.mockResolvedValueOnce({} as any);
    expect(await clipboardHasImage()).toBe(true);
  });

  it("returns false when clipboard has no image", async () => {
    mockReadImage.mockRejectedValueOnce(new Error("No image"));
    expect(await clipboardHasImage()).toBe(false);
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/clipboard.test.ts`
Expected: FAIL — `clipboardHasImage` not found

**Step 3: Write the implementation**

Create `src/lib/clipboard.ts`:

```typescript
import { readImage } from "@tauri-apps/plugin-clipboard-manager";

/**
 * Check if the system clipboard contains an image.
 * Returns true if an image is present, false otherwise.
 */
export async function clipboardHasImage(): Promise<boolean> {
  try {
    await readImage();
    return true;
  } catch {
    return false;
  }
}
```

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/clipboard.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add src/lib/clipboard.ts src/lib/clipboard.test.ts
git commit -m "feat: add clipboard image detection helper (#30)"
```

---

### Task 3: Update key handler to intercept Cmd+V for image paste

**Files:**
- Modify: `src/lib/terminal-keys.ts`
- Modify: `src/lib/terminal-keys.test.ts`

**Step 1: Write the failing tests**

Add these tests to `src/lib/terminal-keys.test.ts`:

```typescript
describe("image paste handling", () => {
  it("blocks Cmd-V on keydown and calls onImagePaste", () => {
    const write = vi.fn();
    const onImagePaste = vi.fn();
    const handler = makeCustomKeyHandler(write, { onImagePaste });

    const result = handler(makeEvent({ key: "v", metaKey: true, type: "keydown" }));

    expect(result).toBe(false);
    expect(onImagePaste).toHaveBeenCalledOnce();
    expect(write).not.toHaveBeenCalled();
  });

  it("blocks Cmd-V on keyup without calling onImagePaste", () => {
    const write = vi.fn();
    const onImagePaste = vi.fn();
    const handler = makeCustomKeyHandler(write, { onImagePaste });

    const result = handler(makeEvent({ key: "v", metaKey: true, type: "keyup" }));

    expect(result).toBe(false);
    expect(onImagePaste).not.toHaveBeenCalled();
  });

  it("blocks Ctrl-V on keydown and calls onImagePaste", () => {
    const write = vi.fn();
    const onImagePaste = vi.fn();
    const handler = makeCustomKeyHandler(write, { onImagePaste });

    const result = handler(makeEvent({ key: "v", ctrlKey: true, type: "keydown" }));

    expect(result).toBe(false);
    expect(onImagePaste).toHaveBeenCalledOnce();
  });

  it("lets Cmd-V through when no onImagePaste callback (backward compat)", () => {
    const write = vi.fn();
    const handler = makeCustomKeyHandler(write);

    const result = handler(makeEvent({ key: "v", metaKey: true, type: "keydown" }));

    expect(result).toBe(true);
  });
});
```

**Step 2: Run tests to verify they fail**

Run: `npx vitest run src/lib/terminal-keys.test.ts`
Expected: FAIL — `makeCustomKeyHandler` doesn't accept options

**Step 3: Update the implementation**

Update `src/lib/terminal-keys.ts`:

```typescript
/**
 * Custom key-event handler for xterm.js terminals.
 *
 * Returns `false` to block xterm from processing the event,
 * `true` to let xterm handle it normally.
 *
 * `sendRawToPty` sends data bypassing tmux's outer terminal parser (for CSI u sequences).
 */

interface KeyHandlerOptions {
  /** Called on Cmd-V / Ctrl-V keydown to handle image paste from clipboard. */
  onImagePaste?: () => void;
}

export function makeCustomKeyHandler(
  sendRawToPty: (data: string) => void,
  options?: KeyHandlerOptions,
) {
  return (event: KeyboardEvent): boolean => {
    // Shift+Enter must be blocked on ALL event types (keydown, keypress, keyup)
    // to prevent xterm from also processing it as a regular Enter (\r).
    // We only send the CSI u sequence on keydown to avoid duplicates.
    // Uses send_raw_to_pty which bypasses tmux's outer terminal parser via
    // `tmux send-keys -H`, since tmux doesn't recognise CSI u from the outer PTY.
    if (event.key === "Enter" && event.shiftKey) {
      if (event.type === "keydown") {
        sendRawToPty("\x1b[13;2u");
      }
      return false;
    }

    // Cmd-V / Ctrl-V: intercept to handle image paste from clipboard.
    // We block xterm's native paste and handle it ourselves so we can
    // check for clipboard images and trigger Claude Code's paste handler.
    if (
      event.key === "v" &&
      (event.metaKey || event.ctrlKey) &&
      options?.onImagePaste
    ) {
      if (event.type === "keydown") {
        options.onImagePaste();
      }
      return false;
    }

    return true;
  };
}
```

**Step 4: Update existing test for backward compatibility**

The existing test "does not intercept Cmd-V (paste handled natively by xterm)" should still pass because it doesn't pass options.

**Step 5: Run all tests to verify they pass**

Run: `npx vitest run src/lib/terminal-keys.test.ts`
Expected: ALL PASS

**Step 6: Commit**

```bash
git add src/lib/terminal-keys.ts src/lib/terminal-keys.test.ts
git commit -m "feat: intercept Cmd-V in key handler for image paste (#30)"
```

---

### Task 4: Wire up image paste in Terminal.svelte

**Files:**
- Modify: `src/lib/Terminal.svelte`

**Step 1: Update Terminal.svelte to handle image paste**

Add the clipboard import and paste handler:

```typescript
// Add import at top
import { clipboardHasImage } from "./clipboard";

// In onMount, replace the attachCustomKeyEventHandler call with:
const writeToPty = (data: string) =>
  invoke("write_to_pty", { sessionId, data });

term.attachCustomKeyEventHandler(
  makeCustomKeyHandler(
    (data) => invoke("send_raw_to_pty", { sessionId, data }),
    {
      onImagePaste: () => {
        // Check clipboard for image, then send bracket paste or text
        handleImagePaste(writeToPty);
      },
    },
  ),
);

// Add this function before onMount:
async function handleImagePaste(
  writeToPty: (data: string) => Promise<unknown>,
) {
  const hasImage = await clipboardHasImage();
  if (hasImage) {
    // Send empty bracket paste to trigger Claude Code's clipboard image reader
    await writeToPty("\x1b[200~\x1b[201~");
  } else {
    // No image — read text from clipboard and send it as bracket paste
    try {
      const text = await navigator.clipboard.readText();
      if (text) {
        await writeToPty("\x1b[200~" + text + "\x1b[201~");
      }
    } catch {
      // Clipboard read failed — nothing to paste
    }
  }
}
```

**Step 2: Verify the app builds**

Run: `npm run build 2>&1 | tail -10`
Expected: Build succeeds

**Step 3: Commit**

```bash
git add src/lib/Terminal.svelte
git commit -m "feat: wire up image paste handling in terminal (#30)"
```

---

### Task 5: Manual testing

**Step 1: Start the dev server**

Run: `npx tauri dev --port 1421` (use different port for separate evaluation)

**Step 2: Test image-only paste**

1. Take a screenshot (Cmd+Shift+4) to put an image in clipboard
2. Focus a Claude session terminal
3. Press Cmd+V
4. Expected: Claude Code should detect the image from clipboard

**Step 3: Test text paste**

1. Copy some text to clipboard
2. Focus a Claude session terminal
3. Press Cmd+V
4. Expected: Text appears in the terminal input

**Step 4: Test text+image paste**

1. Copy an image to clipboard, then also have text context
2. Press Cmd+V
3. Expected: Claude Code detects the image

---

### Task 6: If approach A fails — pivot to approach B

If Claude Code cannot read the system clipboard from within the PTY, change `handleImagePaste` to:
1. Read the image bytes via `readImage()` + `image.rgba()`
2. Save to a temp file via a new Tauri command
3. Send the file path as text to the PTY

This is documented as a contingency — only implement if Task 5 manual testing shows Claude Code doesn't detect the image.
