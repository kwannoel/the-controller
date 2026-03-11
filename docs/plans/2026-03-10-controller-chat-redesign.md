# Controller Chat Redesign Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Add a `g` hotkey to toggle the controller chat panel, and restyle it to match the app's visual language.

**Architecture:** Three layers: (1) new `controllerChatVisible` store + command registration, (2) hotkey handler wiring in HotkeyManager, (3) GlobalChat.svelte restyle + conditional render in App.svelte. The existing `sidebarVisible` / `toggle-sidebar` pattern is followed exactly.

**Tech Stack:** Svelte 5 stores, commands.ts registry, HotkeyManager.svelte, GlobalChat.svelte CSS

---

### Task 1: Add `controllerChatVisible` store + command definition

**Files:**
- Modify: `src/lib/stores.ts:263` (after `sidebarVisible`)
- Modify: `src/lib/commands.ts:16` (CommandId union) and `:90` (commands array)

**Step 1: Write the failing test**

Add to `src/lib/stores.test.ts`:

```typescript
// Add controllerChatVisible to the import at line 14
import {
  // ... existing imports ...
  controllerChatVisible,
} from './stores';
```

```typescript
// Add after the sidebarVisible test (line 82):
it('controllerChatVisible defaults to true', () => {
  expect(get(controllerChatVisible)).toBe(true);
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/stores.test.ts`
Expected: FAIL — `controllerChatVisible` is not exported from stores

**Step 3: Write minimal implementation**

In `src/lib/stores.ts`, add after line 263 (`export const sidebarVisible = ...`):

```typescript
export const controllerChatVisible = writable<boolean>(true);
```

In `src/lib/commands.ts`, add `"toggle-controller-chat"` to the `CommandId` union type:

```typescript
| "toggle-controller-chat";
```

Add to the `commands` array in the Panels section (after the `toggle-sidebar` entry at line 90):

```typescript
{ id: "toggle-controller-chat", key: "g", section: "Panels", description: "Toggle controller chat" },
```

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/stores.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add src/lib/stores.ts src/lib/stores.test.ts src/lib/commands.ts
git commit -m "feat: add controllerChatVisible store and toggle-controller-chat command"
```

---

### Task 2: Wire `g` hotkey in HotkeyManager

**Files:**
- Modify: `src/lib/HotkeyManager.svelte` (import store, add case)
- Modify: `src/lib/HotkeyManager.test.ts` (update existing `g` test)

**Step 1: Write the failing test**

In `src/lib/HotkeyManager.test.ts`, update the import to include `controllerChatVisible`:

```typescript
import { projects, activeSessionId, hotkeyAction, focusTarget, sidebarVisible, controllerChatVisible, expandedProjects, workspaceMode, workspaceModePickerVisible, selectedSessionProvider, type Project, type SessionConfig } from './stores';
```

Add `controllerChatVisible.set(true);` to the `beforeEach` block (after `sidebarVisible.set(true)`).

Replace the `describe('removed g hotkey', ...)` block (lines 362-373) with:

```typescript
describe('g toggles controller chat', () => {
  it('g toggles controllerChatVisible', () => {
    expect(get(controllerChatVisible)).toBe(true);
    pressKey('g');
    expect(get(controllerChatVisible)).toBe(false);
    pressKey('g');
    expect(get(controllerChatVisible)).toBe(true);
  });
});
```

Also update the terminal escape test "Escape then g remains inert in ambient mode" (lines 419-433) — this test asserts `g` does nothing, but now `g` toggles the chat. Replace it:

```typescript
it('Escape then g toggles controller chat in ambient mode', () => {
  pressKey('Escape');

  removeTerminalFocus(xtermEl);
  xtermEl = document.createElement('div');

  pressKey('g');
  expect(get(controllerChatVisible)).toBe(false);
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/HotkeyManager.test.ts`
Expected: FAIL — the `toggle-controller-chat` case is not handled in handleHotkey

**Step 3: Write minimal implementation**

In `src/lib/HotkeyManager.svelte`:

Add `controllerChatVisible` to the import from `./stores` (line 8):

```typescript
import {
  // ... existing imports ...
  controllerChatVisible,
} from "./stores";
```

Add a case in the `handleHotkey` switch, after the `toggle-sidebar` case (after line 314):

```typescript
case "toggle-controller-chat":
  controllerChatVisible.update(v => !v);
  return true;
```

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/HotkeyManager.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add src/lib/HotkeyManager.svelte src/lib/HotkeyManager.test.ts
git commit -m "feat: wire g hotkey to toggle controller chat visibility"
```

---

### Task 3: Conditional render in App.svelte

**Files:**
- Modify: `src/App.svelte`

**Step 1: Add the conditional render**

In `src/App.svelte`, import `controllerChatVisible` from stores (add to line 25 imports):

```typescript
import { ..., controllerChatVisible, ... } from "./lib/stores";
```

Add a `fromStore` binding (after `sidebarVisibleState` around line 33):

```typescript
const controllerChatVisibleState = fromStore(controllerChatVisible);
```

Wrap the `<GlobalChat />` render (line 333) in a conditional:

```svelte
{#if controllerChatVisibleState.current}
  <GlobalChat />
{/if}
```

**Step 2: Verify manually**

Run: `npm run tauri dev`
Press `g` — controller chat should hide. Press `g` again — it should reappear.

**Step 3: Commit**

```bash
git add src/App.svelte
git commit -m "feat: conditionally render controller chat based on visibility store"
```

---

### Task 4: Restyle GlobalChat.svelte

**Files:**
- Modify: `src/lib/GlobalChat.svelte` (template + styles)
- Modify: `src/lib/GlobalChat.test.ts` (update selectors)

**Step 1: Update the test to match new UI**

The existing test at `src/lib/GlobalChat.test.ts` line 112 finds the Send button with `screen.getByRole("button", { name: "Send" })`. Since the redesign removes the visible Send button, update the submit test to use Enter keypress instead.

Replace the submit test's form submission (lines 109-112):

```typescript
const input = screen.getByTestId("controller-chat-input");
await fireEvent.input(input, {
  target: { value: "fetch issue 123" },
});
await fireEvent.keyDown(input, { key: "Enter" });
```

The focus display test (line 73) checks `controller-chat-focus` for "Project Alpha / issue-123.md". The redesigned header still shows this content, so the assertion stays the same.

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/GlobalChat.test.ts`
Expected: FAIL — the Send button no longer exists (test tries to find it)

Wait — actually the test will pass until we change the template. So update the test first, then update the template. The test will fail because it looks for `getByRole("button", { name: "Send" })` which won't exist after the restyle. Let's do both together.

**Step 3: Restyle the component**

Replace the `<aside>` template section of `GlobalChat.svelte` (lines 141-186) with:

```svelte
<aside class="global-chat" data-testid="global-chat">
  <header class="chat-header" data-testid="controller-chat-focus">
    {#if session.focus.project_name}
      <span class="focus-project">{session.focus.project_name}</span>
      {#if session.focus.note_filename}
        <span class="focus-separator">/</span>
        <span class="focus-note">{session.focus.note_filename}</span>
      {/if}
    {:else}
      <span class="focus-empty">No focus</span>
    {/if}
  </header>

  <div class="transcript" data-testid="controller-chat-transcript">
    {#if session.items.length === 0}
      <div class="empty">No messages yet</div>
    {:else}
      {#each session.items as item, index}
        <div class={`item item-${item.kind}`} data-testid={`controller-chat-item-${index}`}>
          {item.text}
        </div>
      {/each}
    {/if}
  </div>

  <div class="composer">
    <textarea
      bind:value={draft}
      rows="2"
      placeholder="Ask the controller..."
      disabled={session.turn_in_progress}
      data-testid="controller-chat-input"
      onkeydown={(e) => {
        if (e.key === "Enter" && !e.shiftKey) {
          e.preventDefault();
          submitMessage();
        }
      }}
    ></textarea>
    {#if session.turn_in_progress}
      <span class="working-indicator">working...</span>
    {/if}
  </div>
</aside>
```

Replace the entire `<style>` block with:

```css
<style>
  .global-chat {
    width: 280px;
    min-width: 280px;
    border-left: 1px solid #313244;
    background: #1e1e2e;
    color: #cdd6f4;
    display: flex;
    flex-direction: column;
  }

  .chat-header {
    padding: 12px 16px;
    border-bottom: 1px solid #313244;
    font-size: 14px;
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: 4px;
    min-height: 20px;
  }

  .focus-project {
    color: #cdd6f4;
  }

  .focus-separator {
    color: #6c7086;
  }

  .focus-note {
    color: #bac2de;
  }

  .focus-empty {
    color: #6c7086;
  }

  .transcript {
    flex: 1;
    overflow-y: auto;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .empty {
    color: #6c7086;
    font-size: 13px;
    text-align: center;
    margin-top: 24px;
  }

  .item {
    padding: 8px 12px;
    font-size: 13px;
    white-space: pre-wrap;
    word-break: break-word;
    border-left: 3px solid transparent;
  }

  .item-user {
    border-left-color: #89b4fa;
  }

  .item-assistant {
    border-left-color: #a6e3a1;
  }

  .item-tool {
    border-left-color: #f9e2af;
  }

  .composer {
    padding: 8px;
    border-top: 1px solid #313244;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  textarea {
    width: 100%;
    resize: none;
    border: 1px solid #45475a;
    border-radius: 4px;
    background: #11111b;
    color: #cdd6f4;
    padding: 8px 10px;
    font: inherit;
    font-size: 13px;
    box-sizing: border-box;
  }

  textarea:disabled {
    opacity: 0.6;
  }

  .working-indicator {
    font-size: 11px;
    color: #6c7086;
    padding: 0 2px;
  }
</style>
```

**Step 4: Update the test**

In `src/lib/GlobalChat.test.ts`, update the submit test. Replace lines 109-112:

```typescript
const input = screen.getByTestId("controller-chat-input");
await fireEvent.input(input, {
  target: { value: "fetch issue 123" },
});
await fireEvent.keyDown(input, { key: "Enter" });
```

**Step 5: Run tests to verify they pass**

Run: `npx vitest run src/lib/GlobalChat.test.ts`
Expected: PASS

**Step 6: Commit**

```bash
git add src/lib/GlobalChat.svelte src/lib/GlobalChat.test.ts
git commit -m "feat: restyle controller chat — left-border accents, compact layout, Enter-to-send"
```

---

### Task 5: Final verification

**Step 1: Run all frontend tests**

Run: `npx vitest run`
Expected: All tests PASS

**Step 2: Visual check**

Run: `npm run tauri dev`
Verify:
- `g` toggles controller chat panel
- `s` still toggles sidebar
- Chat panel matches sidebar styling (same background, borders, header height)
- Messages show colored left borders (blue/green/yellow)
- Enter sends, Shift+Enter adds newline
- "working..." indicator appears during turns
- Empty state shows "No messages yet"

**Step 3: Commit any fixups if needed**
