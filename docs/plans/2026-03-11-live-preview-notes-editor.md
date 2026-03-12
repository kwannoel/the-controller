# Live Preview Notes Editor — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Replace the separate edit/preview/split modes with a single Notion-like editor that renders markdown inline with vim keybindings.

**Architecture:** A CodeMirror `ViewPlugin` walks the markdown syntax tree (from `@codemirror/lang-markdown` / `@lezer/markdown`) and produces `Decoration.replace()` (to hide syntax markers) and `Decoration.mark()` (to style content). When the cursor is on a line, decorations for that line are removed to reveal raw markdown. The plugin is added as a single extension to the existing editor.

**Tech Stack:** CodeMirror 6, `@codemirror/lang-markdown`, `@codemirror/language` (for `syntaxTree`), `@codemirror/view` (for `ViewPlugin`, `Decoration`, `DecorationSet`, `WidgetType`), `@replit/codemirror-vim` (unchanged).

**Lezer markdown node types (from `@lezer/markdown`):**
- Block: `ATXHeading1`–`ATXHeading6`, `FencedCode`, `BulletList`, `OrderedList`, `ListItem`, `Paragraph`
- Inline: `Emphasis`, `StrongEmphasis`, `InlineCode`, `Link`
- Markers: `HeaderMark`, `EmphasisMark`, `CodeMark`, `CodeText`, `CodeInfo`, `LinkMark`, `ListMark`, `URL`, `LinkLabel`, `LinkTitle`

---

### Task 1: Create the live preview ViewPlugin with heading support

**Files:**
- Create: `src/lib/markdownLivePreview.ts`
- Test: `src/lib/markdownLivePreview.test.ts`

This task builds the core plugin infrastructure and handles headings (the simplest element to verify visually).

**Step 1: Write the test file**

Create `src/lib/markdownLivePreview.test.ts`:

```typescript
import { describe, expect, it } from "vitest";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { markdown } from "@codemirror/lang-markdown";
import { markdownLivePreview } from "./markdownLivePreview";

function createView(doc: string, cursorPos?: number): EditorView {
  const state = EditorState.create({
    doc,
    extensions: [markdown(), markdownLivePreview()],
    selection: cursorPos !== undefined ? { anchor: cursorPos } : undefined,
  });
  // EditorView needs a parent to render decorations
  const parent = document.createElement("div");
  return new EditorView({ state, parent });
}

function getDecorations(view: EditorView) {
  // Access the plugin's decorations via the view's plugin field
  const plugin = view.plugin(
    // We need to access the plugin instance — use a query approach
    // by checking decoration classes in the DOM
  );
  // Instead: check the rendered DOM for decoration classes
  return view.dom;
}

describe("markdownLivePreview", () => {
  describe("headings", () => {
    it("applies heading class to ATXHeading lines when cursor is elsewhere", () => {
      const view = createView("# Hello World\n\nsome text", 20); // cursor on "some text"
      const lines = view.dom.querySelectorAll(".cm-line");
      // The heading line should have the decoration class
      expect(lines[0].querySelector(".cm-md-h1")).not.toBeNull();
    });

    it("does not hide heading markers when cursor is on the heading line", () => {
      const view = createView("# Hello World\n\nsome text", 3); // cursor on heading line
      const lines = view.dom.querySelectorAll(".cm-line");
      // No decorations on cursor line — raw markdown visible
      expect(lines[0].querySelector(".cm-md-h1")).toBeNull();
    });

    it("applies different classes for h1 through h3", () => {
      const doc = "# H1\n\n## H2\n\n### H3\n\ntext";
      const view = createView(doc, doc.length - 1); // cursor on last line
      expect(view.dom.querySelector(".cm-md-h1")).not.toBeNull();
      expect(view.dom.querySelector(".cm-md-h2")).not.toBeNull();
      expect(view.dom.querySelector(".cm-md-h3")).not.toBeNull();
    });
  });
});
```

**Step 2: Run the test to verify it fails**

Run: `npx vitest run src/lib/markdownLivePreview.test.ts`
Expected: FAIL — module `./markdownLivePreview` not found.

**Step 3: Write the core plugin with heading decorations**

Create `src/lib/markdownLivePreview.ts`:

```typescript
import {
  Decoration,
  DecorationSet,
  EditorView,
  ViewPlugin,
  ViewUpdate,
} from "@codemirror/view";
import { syntaxTree } from "@codemirror/language";
import { RangeSetBuilder } from "@codemirror/state";

/** CSS classes applied by mark decorations. */
const headingMark = {
  1: Decoration.mark({ class: "cm-md-h1" }),
  2: Decoration.mark({ class: "cm-md-h2" }),
  3: Decoration.mark({ class: "cm-md-h3" }),
  4: Decoration.mark({ class: "cm-md-h4" }),
  5: Decoration.mark({ class: "cm-md-h5" }),
  6: Decoration.mark({ class: "cm-md-h6" }),
} as Record<number, Decoration>;

const headerMarkerHide = Decoration.replace({});

function cursorLineRanges(view: EditorView): Set<number> {
  const lines = new Set<number>();
  for (const range of view.state.selection.ranges) {
    const startLine = view.state.doc.lineAt(range.from).number;
    const endLine = view.state.doc.lineAt(range.to).number;
    for (let l = startLine; l <= endLine; l++) {
      lines.add(l);
    }
  }
  return lines;
}

function buildDecorations(view: EditorView): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  const cursorLines = cursorLineRanges(view);
  const tree = syntaxTree(view.state);

  // Collect decorations in document order (required by RangeSetBuilder)
  const decorations: { from: number; to: number; deco: Decoration }[] = [];

  tree.iterate({
    enter(node) {
      const lineStart = view.state.doc.lineAt(node.from).number;
      const lineEnd = view.state.doc.lineAt(node.to).number;

      // Check if any line of this node overlaps with cursor lines
      let onCursorLine = false;
      for (let l = lineStart; l <= lineEnd; l++) {
        if (cursorLines.has(l)) {
          onCursorLine = true;
          break;
        }
      }
      if (onCursorLine) return;

      const name = node.name;

      // ATXHeading1..6 — style the whole heading line
      const headingMatch = name.match(/^ATXHeading(\d)$/);
      if (headingMatch) {
        const level = parseInt(headingMatch[1]);
        decorations.push({ from: node.from, to: node.to, deco: headingMark[level] });
      }

      // HeaderMark — hide the `# ` prefix (and the space after)
      if (name === "HeaderMark") {
        // Hide from marker start to marker end + 1 (the trailing space)
        const hideEnd = Math.min(node.to + 1, view.state.doc.length);
        decorations.push({ from: node.from, to: hideEnd, deco: headerMarkerHide });
      }
    },
  });

  // Sort by `from` position (RangeSetBuilder requires sorted order)
  decorations.sort((a, b) => a.from - b.from || a.to - b.to);
  for (const { from, to, deco } of decorations) {
    builder.add(from, to, deco);
  }

  return builder.finish();
}

const livePreviewPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = buildDecorations(view);
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.selectionSet || update.viewportChanged) {
        this.decorations = buildDecorations(update.view);
      }
    }
  },
  { decorations: (v) => v.decorations },
);

/** Extension that renders markdown live-preview decorations. */
export function markdownLivePreview() {
  return livePreviewPlugin;
}
```

**Step 4: Run the test to verify it passes**

Run: `npx vitest run src/lib/markdownLivePreview.test.ts`
Expected: PASS. If the DOM-based tests don't work in jsdom (CodeMirror needs a real DOM for decorations), convert to unit tests that call `buildDecorations` directly and inspect the `DecorationSet`. Adjust the test approach as needed — the key behavior to verify is that decorations are produced for non-cursor lines and skipped for cursor lines.

**Step 5: Commit**

```bash
git add src/lib/markdownLivePreview.ts src/lib/markdownLivePreview.test.ts
git commit -m "feat: add live preview ViewPlugin with heading decorations"
```

---

### Task 2: Add inline formatting decorations (bold, italic, inline code)

**Files:**
- Modify: `src/lib/markdownLivePreview.ts`
- Modify: `src/lib/markdownLivePreview.test.ts`

**Step 1: Add tests for inline formatting**

Append to the test file:

```typescript
describe("inline formatting", () => {
  it("applies bold class and hides markers when cursor is elsewhere", () => {
    const view = createView("**bold text**\n\nother", 18);
    expect(view.dom.querySelector(".cm-md-strong")).not.toBeNull();
  });

  it("applies italic class and hides markers when cursor is elsewhere", () => {
    const view = createView("*italic text*\n\nother", 18);
    expect(view.dom.querySelector(".cm-md-em")).not.toBeNull();
  });

  it("applies inline code class and hides backticks when cursor is elsewhere", () => {
    const view = createView("`code`\n\nother", 10);
    expect(view.dom.querySelector(".cm-md-code")).not.toBeNull();
  });

  it("shows raw markdown when cursor is on the formatted line", () => {
    const view = createView("**bold text**\n\nother", 3);
    expect(view.dom.querySelector(".cm-md-strong")).toBeNull();
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/markdownLivePreview.test.ts`
Expected: FAIL — no `.cm-md-strong` / `.cm-md-em` / `.cm-md-code` classes produced.

**Step 3: Add inline decoration logic**

In `markdownLivePreview.ts`, add these decoration constants:

```typescript
const strongMark = Decoration.mark({ class: "cm-md-strong" });
const emphasisMark = Decoration.mark({ class: "cm-md-em" });
const inlineCodeMark = Decoration.mark({ class: "cm-md-code" });
const syntaxHide = Decoration.replace({});
```

In the `tree.iterate` `enter` callback, add handling for:

```typescript
// StrongEmphasis — bold
if (name === "StrongEmphasis") {
  decorations.push({ from: node.from, to: node.to, deco: strongMark });
}

// EmphasisMark — hide * or ** markers
if (name === "EmphasisMark") {
  decorations.push({ from: node.from, to: node.to, deco: syntaxHide });
}

// Emphasis — italic
if (name === "Emphasis") {
  decorations.push({ from: node.from, to: node.to, deco: emphasisMark });
}

// InlineCode — styled code span
if (name === "InlineCode") {
  decorations.push({ from: node.from, to: node.to, deco: inlineCodeMark });
}

// CodeMark (backticks for inline code) — hide
if (name === "CodeMark") {
  decorations.push({ from: node.from, to: node.to, deco: syntaxHide });
}
```

Note: `EmphasisMark` is a child of both `Emphasis` and `StrongEmphasis`. The `syntaxHide` replace decoration will hide the `*`/`**` markers, while the parent mark decoration styles the content. Since replace decorations take precedence within their range, the markers will be hidden and the text content will be styled.

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/markdownLivePreview.test.ts`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/lib/markdownLivePreview.ts src/lib/markdownLivePreview.test.ts
git commit -m "feat: add bold, italic, and inline code decorations to live preview"
```

---

### Task 3: Add link decorations

**Files:**
- Modify: `src/lib/markdownLivePreview.ts`
- Modify: `src/lib/markdownLivePreview.test.ts`

Links in the syntax tree look like:
```
Link
  LinkMark: [
  (inline content)
  LinkMark: ]
  LinkMark: (
  URL: https://example.com
  LinkMark: )
```

We want to: hide `[`, `](url)`, and style the visible text as a link.

**Step 1: Add link tests**

```typescript
describe("links", () => {
  it("styles link text and hides markdown syntax when cursor is elsewhere", () => {
    const view = createView("[click here](https://example.com)\n\nother", 38);
    expect(view.dom.querySelector(".cm-md-link")).not.toBeNull();
  });

  it("shows raw markdown when cursor is on the link line", () => {
    const view = createView("[click here](https://example.com)\n\nother", 5);
    expect(view.dom.querySelector(".cm-md-link")).toBeNull();
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/markdownLivePreview.test.ts`
Expected: FAIL.

**Step 3: Add link decoration logic**

```typescript
const linkMark = Decoration.mark({ class: "cm-md-link" });

// In the tree.iterate enter callback:

// Link — style the whole link node
if (name === "Link") {
  decorations.push({ from: node.from, to: node.to, deco: linkMark });
}

// LinkMark — hide [ ] ( )
if (name === "LinkMark") {
  decorations.push({ from: node.from, to: node.to, deco: syntaxHide });
}

// URL — hide the URL inside the parens
if (name === "URL") {
  decorations.push({ from: node.from, to: node.to, deco: syntaxHide });
}
```

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/markdownLivePreview.test.ts`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/lib/markdownLivePreview.ts src/lib/markdownLivePreview.test.ts
git commit -m "feat: add link decorations to live preview"
```

---

### Task 4: Add list item decorations

**Files:**
- Modify: `src/lib/markdownLivePreview.ts`
- Modify: `src/lib/markdownLivePreview.test.ts`

List items have a `ListMark` child (`-` or `*`). We replace the marker with a bullet widget and style the list item.

**Step 1: Add list tests**

```typescript
describe("lists", () => {
  it("replaces list marker with bullet when cursor is elsewhere", () => {
    const view = createView("- item one\n- item two\n\nother", 25);
    expect(view.dom.querySelector(".cm-md-list-bullet")).not.toBeNull();
  });

  it("shows raw markdown when cursor is on a list line", () => {
    const view = createView("- item one\n\nother", 3);
    expect(view.dom.querySelector(".cm-md-list-bullet")).toBeNull();
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/markdownLivePreview.test.ts`
Expected: FAIL.

**Step 3: Add list decoration logic**

```typescript
import { WidgetType } from "@codemirror/view";

class BulletWidget extends WidgetType {
  toDOM() {
    const span = document.createElement("span");
    span.className = "cm-md-list-bullet";
    span.textContent = "\u2022 ";  // bullet character + space
    return span;
  }
}

const bulletWidget = Decoration.replace({ widget: new BulletWidget() });

// In the tree.iterate enter callback:

// ListMark — replace `- ` with bullet widget
if (name === "ListMark") {
  // Hide the marker and trailing space
  const hideEnd = Math.min(node.to + 1, view.state.doc.length);
  decorations.push({ from: node.from, to: hideEnd, deco: bulletWidget });
}
```

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/markdownLivePreview.test.ts`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/lib/markdownLivePreview.ts src/lib/markdownLivePreview.test.ts
git commit -m "feat: add list bullet decorations to live preview"
```

---

### Task 5: Add fenced code block decorations

**Files:**
- Modify: `src/lib/markdownLivePreview.ts`
- Modify: `src/lib/markdownLivePreview.test.ts`

Fenced code blocks have structure:
```
FencedCode
  CodeMark: ``` (opening fence)
  CodeInfo: language (optional)
  CodeText: actual code content
  CodeMark: ``` (closing fence)
```

We hide the fence lines (CodeMark + CodeInfo) and style the CodeText with a code block background. For code blocks, we apply decorations even on cursor lines (the styled block background should persist), but we reveal fence lines when cursor is on them.

**Step 1: Add code block tests**

```typescript
describe("code blocks", () => {
  it("styles code block content and hides fences when cursor is elsewhere", () => {
    const doc = "text\n\n```js\nconst x = 1;\n```\n\nother";
    const view = createView(doc, doc.length - 1);
    expect(view.dom.querySelector(".cm-md-codeblock")).not.toBeNull();
  });

  it("shows fence lines when cursor is on them", () => {
    const doc = "text\n\n```js\nconst x = 1;\n```\n\nother";
    const view = createView(doc, 7); // cursor on opening fence line
    // The fence line should be visible (no replace decoration on cursor line)
    expect(view.dom.querySelector(".cm-md-codeblock")).not.toBeNull(); // code content still styled
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/markdownLivePreview.test.ts`
Expected: FAIL.

**Step 3: Add code block decoration logic**

```typescript
const codeBlockMark = Decoration.mark({ class: "cm-md-codeblock" });
const codeBlockLine = Decoration.line({ class: "cm-md-codeblock-line" });

// In the tree.iterate enter callback:

// FencedCode — style the whole block
if (name === "FencedCode") {
  // Apply line decorations for background styling on each line
  const startLine = view.state.doc.lineAt(node.from).number;
  const endLine = view.state.doc.lineAt(node.to).number;
  for (let l = startLine; l <= endLine; l++) {
    const line = view.state.doc.line(l);
    decorations.push({ from: line.from, to: line.from, deco: codeBlockLine });
  }
}

// CodeInfo — hide language tag (only when not on cursor line, already handled by the outer check)
if (name === "CodeInfo") {
  decorations.push({ from: node.from, to: node.to, deco: syntaxHide });
}

// CodeMark inside FencedCode — hide fence markers (``` lines)
// Note: CodeMark is also used for inline code backticks.
// For fenced code, CodeMark spans an entire ``` line.
// We distinguish by checking if the CodeMark is on its own line.
if (name === "CodeMark") {
  const line = view.state.doc.lineAt(node.from);
  const isFenceMark = line.text.trim().startsWith("```");
  if (isFenceMark) {
    // Hide the entire fence line content
    decorations.push({ from: node.from, to: node.to, deco: syntaxHide });
  } else {
    // Inline code backtick — already handled above
    decorations.push({ from: node.from, to: node.to, deco: syntaxHide });
  }
}
```

Note: The `CodeMark` handling for inline code backticks was already added in Task 2. Refactor so `CodeMark` is only handled once: detect fence marks by checking if the parent node is `FencedCode` (use `node.node.parent?.name === "FencedCode"`).

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/markdownLivePreview.test.ts`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/lib/markdownLivePreview.ts src/lib/markdownLivePreview.test.ts
git commit -m "feat: add fenced code block decorations to live preview"
```

---

### Task 6: Integrate the plugin into CodeMirrorNoteEditor and add CSS

**Files:**
- Modify: `src/lib/CodeMirrorNoteEditor.svelte`

**Step 1: Add the plugin to the extensions array**

In `CodeMirrorNoteEditor.svelte`, import and add the plugin:

```typescript
import { markdownLivePreview } from "./markdownLivePreview";
```

In the `buildState` function, add `markdownLivePreview()` to the extensions array:

```typescript
extensions: [
  vim(),
  drawSelection(),
  markdown(),
  markdownLivePreview(),  // <-- add here
  EditorView.lineWrapping,
  // ... rest unchanged
],
```

**Step 2: Add CSS for the decoration classes**

In the `<style>` section of `CodeMirrorNoteEditor.svelte`, add:

```css
/* Live preview heading styles — match preview pane */
.note-code-editor :global(.cm-md-h1) {
  font-size: 24px;
  font-weight: 700;
  font-family: var(--font-sans, sans-serif);
}

.note-code-editor :global(.cm-md-h2) {
  font-size: 20px;
  font-weight: 600;
  font-family: var(--font-sans, sans-serif);
}

.note-code-editor :global(.cm-md-h3) {
  font-size: 16px;
  font-weight: 600;
  font-family: var(--font-sans, sans-serif);
}

.note-code-editor :global(.cm-md-h4),
.note-code-editor :global(.cm-md-h5),
.note-code-editor :global(.cm-md-h6) {
  font-size: 14px;
  font-weight: 600;
  font-family: var(--font-sans, sans-serif);
}

/* Inline formatting */
.note-code-editor :global(.cm-md-strong) {
  font-weight: 700;
}

.note-code-editor :global(.cm-md-em) {
  font-style: italic;
}

.note-code-editor :global(.cm-md-code) {
  background: var(--bg-surface);
  padding: 2px 5px;
  border-radius: 3px;
  font-family: var(--font-mono);
  font-size: 13px;
}

/* Links */
.note-code-editor :global(.cm-md-link) {
  color: var(--text-emphasis);
  text-decoration: none;
}

.note-code-editor :global(.cm-md-link:hover) {
  text-decoration: underline;
}

/* List bullets */
.note-code-editor :global(.cm-md-list-bullet) {
  color: var(--text-secondary);
}

/* Fenced code blocks */
.note-code-editor :global(.cm-md-codeblock-line) {
  background: var(--bg-surface);
}

.note-code-editor :global(.cm-md-codeblock-line:first-of-type) {
  border-radius: 6px 6px 0 0;
}

.note-code-editor :global(.cm-md-codeblock-line:last-of-type) {
  border-radius: 0 0 6px 6px;
}
```

**Step 3: Switch the scroller font to sans-serif**

Since the editor now renders like a document (not raw code), change the font from mono to sans:

```css
.note-code-editor :global(.cm-scroller) {
  font-family: var(--font-sans, sans-serif);
  line-height: 1.6;
}
```

Code blocks and inline code will still use `var(--font-mono)` via their specific decoration classes.

**Step 4: Verify the plugin loads without errors**

Run: `npx vitest run src/lib/markdownLivePreview.test.ts`
Then: `npm run tauri dev` — open a note and visually confirm headings render large, bold renders bold, etc. Move cursor to a formatted line and confirm raw markdown appears.

**Step 5: Commit**

```bash
git add src/lib/CodeMirrorNoteEditor.svelte
git commit -m "feat: integrate live preview plugin and add styling"
```

---

### Task 7: Remove the edit/preview/split UI from NotesEditor

**Files:**
- Modify: `src/lib/NotesEditor.svelte`
- Modify: `src/lib/NotesEditor.test.ts`

**Step 1: Update the tests**

Remove the tests that reference view mode controls, split mode, and preview pane. Update them to test that the editor is always visible:

- Delete test: `"renders explicit edit, preview, and split mode controls"`
- Delete test: `"shows both the editor and rendered markdown in split mode"`
- Delete test: `"cycles edit, preview, and split modes through the notes hotkey action"`
- Keep tests: escape handling, out-of-order note loading, editor mounting

Remove `noteViewMode` from imports and `beforeEach` in the test file.

**Step 2: Run tests to see which fail**

Run: `npx vitest run src/lib/NotesEditor.test.ts`
Expected: 3 tests deleted, remaining tests should still pass.

**Step 3: Simplify NotesEditor.svelte**

Remove from the script section:
- `import { renderMarkdown } from "./markdown"`
- `noteViewMode` and `type NoteViewMode` from imports
- `viewModeState`, `currentViewMode`, `showsEditor`, `showsPreview` derived values
- `renderedHtml` derived
- `setViewMode()` function
- The `toggle-note-preview` hotkey handler

Remove from the template:
- The `.view-mode-controls` div and its three buttons
- The conditional `{#if showsPreview}` block and `.preview` div
- The `class:split` bindings
- The `{#if showsEditor}` conditional — editor is always shown

Remove from the styles:
- `.view-mode-controls`, `.view-mode-button` styles
- `.preview` and all `.preview :global(...)` styles
- `.editor-body.split` style

The editor body becomes simply:

```svelte
<div class="editor-body" class:focused={editorFocused}>
  <CodeMirrorNoteEditor
    value={content}
    focused={editorFocused}
    entryKey={editorEntryKey}
    onChange={handleEditorChange}
    onModeChange={(mode) => { editorMode = mode; }}
    onEscape={handleEditorEscape}
    onAiChat={(request) => { aiChatRequest = request; }}
  />
</div>
```

**Step 4: Run all tests**

Run: `npx vitest run`
Expected: all tests pass.

**Step 5: Commit**

```bash
git add src/lib/NotesEditor.svelte src/lib/NotesEditor.test.ts
git commit -m "feat: remove edit/preview/split UI in favor of live preview"
```

---

### Task 8: Visual QA and polish

**Files:**
- Possibly adjust: `src/lib/markdownLivePreview.ts`, `src/lib/CodeMirrorNoteEditor.svelte`

**Step 1: Run the app and test with a sample note**

Run: `npm run tauri dev`

Create or open a note with all supported elements:

```markdown
# Main Heading

## Subheading

Some text with **bold** and *italic* and `inline code`.

- First item
- Second item
- Third item

[Example link](https://example.com)

```js
const hello = "world";
console.log(hello);
```

### Another heading

Regular paragraph text.
```

**Step 2: Verify each element**

Check these behaviors:
- [ ] H1 renders at 24px, bold, sans-serif
- [ ] H2 renders at 20px, semibold, sans-serif
- [ ] H3 renders at 16px, semibold, sans-serif
- [ ] `#` markers hidden when cursor is elsewhere
- [ ] `#` markers appear when cursor moves to heading line
- [ ] Bold text appears bold, `**` markers hidden
- [ ] Italic text appears italic, `*` markers hidden
- [ ] Inline code has background, backticks hidden
- [ ] List items show bullets instead of `-`
- [ ] Code blocks have background, fences hidden
- [ ] Links show styled text, URL hidden
- [ ] Moving cursor to any formatted line reveals raw markdown
- [ ] Vim keybindings work normally (hjkl, i, v, dd, etc.)
- [ ] `ga` in visual mode still triggers AI chat
- [ ] Escape behavior unchanged (insert→normal, normal→note list)
- [ ] Save still works (auto-save, escape-save)

**Step 3: Fix any visual issues found**

Adjust CSS values or decoration logic as needed. Common issues:
- Line height inconsistency between heading and regular lines
- Code block background not spanning full width (use line decorations)
- Bullet character alignment

**Step 4: Commit any fixes**

```bash
git add -u
git commit -m "fix: polish live preview styling"
```

---

### Task 9: Clean up unused code

**Files:**
- Modify: `src/lib/stores.ts` — check if `NoteViewMode` and `noteViewMode` are used elsewhere
- Possibly modify: other files that reference `noteViewMode`

**Step 1: Check for remaining references**

Run: `grep -r "noteViewMode\|NoteViewMode" src/`

If `noteViewMode` is only referenced in the test setup (which was already cleaned up) and the store definition, it can be removed. If other components reference it, leave it.

**Step 2: Remove if safe**

If no other consumers, remove from `stores.ts`:
```typescript
// Remove these lines:
export type NoteViewMode = "edit" | "preview" | "split";
export const noteViewMode = writable<NoteViewMode>("edit");
```

**Step 3: Run all tests**

Run: `npx vitest run`
Expected: PASS.

**Step 4: Commit**

```bash
git add -u
git commit -m "chore: remove unused noteViewMode store"
```
