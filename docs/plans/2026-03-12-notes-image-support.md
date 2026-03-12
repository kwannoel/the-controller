# Notes Image Support Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Support images in notes mode — paste, drag & drop, manual markdown, and AI-inserted images rendered inline via the Notion-like live preview.

**Architecture:** Images saved to `~/.the-controller/notes/{project}/assets/{uuid8}.{ext}` via Rust backend. The CodeMirror live preview plugin renders `![alt](path)` as inline `<img>` widgets. Paste/drop handlers in the editor component save image bytes through a Tauri command and insert markdown at cursor. External URLs (http/https) pass through directly.

**Tech Stack:** Rust (Tauri v2 commands, fs, uuid), TypeScript (CodeMirror 6 WidgetType decorations), Svelte 5 (props/events), `@tauri-apps/api/core` (convertFileSrc)

---

### Task 1: Rust image storage functions

**Files:**
- Modify: `src-tauri/src/notes.rs:239` (before `#[cfg(test)]`)
- Modify: `src-tauri/src/notes.rs:242-244` (test module)

**Step 1: Write failing tests**

Add these tests inside the existing `mod tests` block in `src-tauri/src/notes.rs` (after the last test, before the closing `}`):

```rust
    #[test]
    fn test_save_note_image_creates_assets_dir_and_file() {
        let tmp = TempDir::new().unwrap();
        let bytes = vec![0x89, 0x50, 0x4E, 0x47]; // fake PNG header
        let relative_path = save_note_image(tmp.path(), "proj", &bytes, "png").unwrap();
        assert!(relative_path.starts_with("assets/"));
        assert!(relative_path.ends_with(".png"));

        // File should exist on disk
        let full_path = notes_dir_with_base(tmp.path(), "proj").join(&relative_path);
        assert!(full_path.exists());
        assert_eq!(fs::read(&full_path).unwrap(), bytes);
    }

    #[test]
    fn test_save_note_image_rejects_invalid_extension() {
        let tmp = TempDir::new().unwrap();
        let result = save_note_image(tmp.path(), "proj", &[1, 2, 3], "exe");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_save_note_image_unique_filenames() {
        let tmp = TempDir::new().unwrap();
        let bytes = vec![1, 2, 3];
        let path1 = save_note_image(tmp.path(), "proj", &bytes, "png").unwrap();
        let path2 = save_note_image(tmp.path(), "proj", &bytes, "png").unwrap();
        assert_ne!(path1, path2);
    }

    #[test]
    fn test_resolve_note_asset_path_valid() {
        let tmp = TempDir::new().unwrap();
        let bytes = vec![1, 2, 3];
        let relative = save_note_image(tmp.path(), "proj", &bytes, "png").unwrap();
        let abs = resolve_note_asset_path(tmp.path(), "proj", &relative).unwrap();
        assert!(abs.exists());
        assert!(abs.is_absolute());
    }

    #[test]
    fn test_resolve_note_asset_path_rejects_traversal() {
        let tmp = TempDir::new().unwrap();
        let result = resolve_note_asset_path(tmp.path(), "proj", "../../../etc/passwd");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
    }
```

**Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test notes::tests --no-run 2>&1 | head -20`
Expected: Compilation error — `save_note_image` and `resolve_note_asset_path` not defined.

**Step 3: Write implementation**

Add these two functions in `src-tauri/src/notes.rs` just before the `#[cfg(test)]` line (line 241):

```rust
const ALLOWED_IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];

/// Validate that a relative asset path (e.g. "assets/foo.png") is safe.
fn validate_asset_path(relative_path: &str) -> std::io::Result<()> {
    if relative_path.contains("..") || relative_path.contains('\\') || relative_path.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid asset path: {}", relative_path),
        ));
    }
    Ok(())
}

/// Save image bytes to the project's assets directory.
/// Returns the relative path (e.g. "assets/a1b2c3d4.png").
pub fn save_note_image(
    base: &std::path::Path,
    project_name: &str,
    image_bytes: &[u8],
    extension: &str,
) -> std::io::Result<String> {
    let ext_lower = extension.to_lowercase();
    if !ALLOWED_IMAGE_EXTENSIONS.contains(&ext_lower.as_str()) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("unsupported image extension: {}", extension),
        ));
    }

    let assets_dir = notes_dir_with_base(base, project_name).join("assets");
    fs::create_dir_all(&assets_dir)?;

    let short_id = &Uuid::new_v4().to_string()[..8];
    let filename = format!("{}.{}", short_id, ext_lower);
    fs::write(assets_dir.join(&filename), image_bytes)?;

    Ok(format!("assets/{}", filename))
}

/// Resolve a relative asset path to an absolute filesystem path.
/// Validates the path does not escape the notes directory.
pub fn resolve_note_asset_path(
    base: &std::path::Path,
    project_name: &str,
    relative_path: &str,
) -> std::io::Result<PathBuf> {
    validate_asset_path(relative_path)?;
    let full_path = notes_dir_with_base(base, project_name).join(relative_path);
    Ok(full_path)
}
```

**Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test notes::tests -- --nocapture`
Expected: All tests pass, including the 5 new ones.

**Step 5: Commit**

```bash
git add src-tauri/src/notes.rs
git commit -m "feat(notes): add save_note_image and resolve_note_asset_path"
```

---

### Task 2: Tauri commands for image storage

**Files:**
- Modify: `src-tauri/src/commands.rs:1528` (after `delete_note`, before `send_note_ai_chat`)
- Modify: `src-tauri/src/lib.rs:115` (command registration, after `commands::delete_note`)

**Step 1: Add Tauri command wrappers**

In `src-tauri/src/commands.rs`, add these two commands after `delete_note` (after line 1528) and before `send_note_ai_chat`:

```rust
#[tauri::command]
pub fn save_note_image(
    state: State<'_, AppState>,
    project_name: String,
    image_bytes: Vec<u8>,
    extension: String,
) -> Result<String, String> {
    let base_dir = state
        .storage
        .lock()
        .map_err(|e| e.to_string())?
        .base_dir();
    crate::notes::save_note_image(&base_dir, &project_name, &image_bytes, &extension)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn resolve_note_asset_path(
    state: State<'_, AppState>,
    project_name: String,
    relative_path: String,
) -> Result<String, String> {
    let base_dir = state
        .storage
        .lock()
        .map_err(|e| e.to_string())?
        .base_dir();
    crate::notes::resolve_note_asset_path(&base_dir, &project_name, &relative_path)
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}
```

**Step 2: Register the new commands**

In `src-tauri/src/lib.rs`, add these two lines in the `generate_handler!` macro after `commands::delete_note,` (line 115):

```rust
            commands::save_note_image,
            commands::resolve_note_asset_path,
```

**Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: Compiles without errors.

**Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(notes): add Tauri commands for image save and path resolution"
```

---

### Task 3: Live preview image rendering

**Files:**
- Modify: `src/lib/markdownLivePreview.ts`
- Modify: `src/lib/markdownLivePreview.test.ts`

**Step 1: Write failing test**

Add this test block inside the `describe("markdownLivePreview", ...)` in `src/lib/markdownLivePreview.test.ts`:

```typescript
  describe("images", () => {
    it("replaces image syntax with img widget when cursor is elsewhere", () => {
      const resolver = (path: string) => `file://${path}`;
      const view = createViewWithResolver("![alt text](assets/photo.png)\n\nother text", resolver, 35);
      expect(view.dom.querySelector(".cm-md-image")).not.toBeNull();
      const img = view.dom.querySelector(".cm-md-image img") as HTMLImageElement;
      expect(img).not.toBeNull();
      expect(img.src).toContain("assets/photo.png");
    });

    it("shows raw markdown when cursor is on the image line", () => {
      const resolver = (path: string) => `file://${path}`;
      const view = createViewWithResolver("![alt text](assets/photo.png)\n\nother text", resolver, 5);
      expect(view.dom.querySelector(".cm-md-image")).toBeNull();
    });

    it("passes through http URLs without resolver", () => {
      const resolver = (_path: string) => null;
      const view = createViewWithResolver("![alt](https://example.com/img.png)\n\nother", resolver, 40);
      const img = view.dom.querySelector(".cm-md-image img") as HTMLImageElement;
      expect(img).not.toBeNull();
      expect(img.src).toBe("https://example.com/img.png");
    });
  });
```

Also update the `createView` helper and add `createViewWithResolver`:

```typescript
function createViewWithResolver(
  doc: string,
  resolveImageSrc: (path: string) => string | null,
  cursorPos?: number,
): EditorView {
  const state = EditorState.create({
    doc,
    extensions: [markdown(), markdownLivePreview({ resolveImageSrc })],
    selection: cursorPos !== undefined ? { anchor: cursorPos } : undefined,
  });
  const parent = document.createElement("div");
  return new EditorView({ state, parent });
}
```

**Step 2: Run tests to verify they fail**

Run: `npx vitest run src/lib/markdownLivePreview.test.ts`
Expected: FAIL — `markdownLivePreview` doesn't accept options.

**Step 3: Implement image rendering in the live preview plugin**

Replace the full content of `src/lib/markdownLivePreview.ts`:

```typescript
import {
  Decoration,
  type DecorationSet,
  EditorView,
  ViewPlugin,
  type ViewUpdate,
  WidgetType,
} from "@codemirror/view";
import { syntaxTree } from "@codemirror/language";
import { RangeSetBuilder, type EditorState, Facet } from "@codemirror/state";

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

const strongMark = Decoration.mark({ class: "cm-md-strong" });
const emphasisMark = Decoration.mark({ class: "cm-md-em" });
const inlineCodeMark = Decoration.mark({ class: "cm-md-code" });
const syntaxHide = Decoration.replace({});
const linkMark = Decoration.mark({ class: "cm-md-link" });

class BulletWidget extends WidgetType {
  toDOM() {
    const span = document.createElement("span");
    span.className = "cm-md-list-bullet";
    span.textContent = "\u2022 ";
    return span;
  }
}

const bulletWidget = Decoration.replace({ widget: new BulletWidget() });
const codeBlockLine = Decoration.line({ class: "cm-md-codeblock-line" });

export type ImageSrcResolver = (path: string) => string | null;

const imageResolverFacet = Facet.define<ImageSrcResolver, ImageSrcResolver>({
  combine: (values) => values[values.length - 1] ?? (() => null),
});

class ImageWidget extends WidgetType {
  constructor(readonly src: string, readonly alt: string) {
    super();
  }
  eq(other: ImageWidget) {
    return this.src === other.src && this.alt === other.alt;
  }
  toDOM() {
    const wrapper = document.createElement("div");
    wrapper.className = "cm-md-image";
    const img = document.createElement("img");
    img.src = this.src;
    img.alt = this.alt;
    img.style.maxWidth = "100%";
    img.style.height = "auto";
    img.style.borderRadius = "4px";
    img.style.display = "block";
    img.style.margin = "4px 0";
    img.draggable = false;
    wrapper.appendChild(img);
    return wrapper;
  }
}

function resolveImageUrl(rawUrl: string, state: EditorState): string | null {
  if (rawUrl.startsWith("http://") || rawUrl.startsWith("https://")) {
    return rawUrl;
  }
  const resolver = state.facet(imageResolverFacet);
  return resolver(rawUrl);
}

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

  const decorations: { from: number; to: number; deco: Decoration }[] = [];

  tree.iterate({
    enter(node) {
      const lineStart = view.state.doc.lineAt(node.from).number;
      const lineEnd = view.state.doc.lineAt(node.to).number;

      let onCursorLine = false;
      for (let l = lineStart; l <= lineEnd; l++) {
        if (cursorLines.has(l)) {
          onCursorLine = true;
          break;
        }
      }
      if (onCursorLine) return;

      const name = node.name;

      if (name === "Image") {
        const text = view.state.doc.sliceString(node.from, node.to);
        const match = text.match(/^!\[([^\]]*)\]\(([^)]+)\)$/);
        if (match) {
          const alt = match[1];
          const rawUrl = match[2];
          const src = resolveImageUrl(rawUrl, view.state);
          if (src) {
            decorations.push({
              from: node.from,
              to: node.to,
              deco: Decoration.replace({ widget: new ImageWidget(src, alt) }),
            });
            return;
          }
        }
      }

      const headingMatch = name.match(/^ATXHeading(\d)$/);
      if (headingMatch) {
        const level = parseInt(headingMatch[1]);
        decorations.push({
          from: node.from,
          to: node.to,
          deco: headingMark[level],
        });
      }

      if (name === "HeaderMark") {
        const hideEnd = Math.min(node.to + 1, view.state.doc.length);
        decorations.push({
          from: node.from,
          to: hideEnd,
          deco: headerMarkerHide,
        });
      }

      if (name === "StrongEmphasis") {
        decorations.push({ from: node.from, to: node.to, deco: strongMark });
      }
      if (name === "Emphasis") {
        decorations.push({ from: node.from, to: node.to, deco: emphasisMark });
      }
      if (name === "EmphasisMark") {
        decorations.push({ from: node.from, to: node.to, deco: syntaxHide });
      }
      if (name === "InlineCode") {
        decorations.push({ from: node.from, to: node.to, deco: inlineCodeMark });
      }
      if (name === "CodeMark") {
        decorations.push({ from: node.from, to: node.to, deco: syntaxHide });
      }

      if (name === "Link") {
        decorations.push({ from: node.from, to: node.to, deco: linkMark });
      }
      if (name === "LinkMark") {
        decorations.push({ from: node.from, to: node.to, deco: syntaxHide });
      }
      if (name === "URL") {
        decorations.push({ from: node.from, to: node.to, deco: syntaxHide });
      }

      if (name === "ListMark") {
        const hideEnd = Math.min(node.to + 1, view.state.doc.length);
        decorations.push({ from: node.from, to: hideEnd, deco: bulletWidget });
      }

      if (name === "FencedCode") {
        const startLine = view.state.doc.lineAt(node.from).number;
        const endLine = view.state.doc.lineAt(node.to).number;
        for (let l = startLine; l <= endLine; l++) {
          const line = view.state.doc.line(l);
          decorations.push({ from: line.from, to: line.from, deco: codeBlockLine });
        }
      }
      if (name === "CodeInfo") {
        decorations.push({ from: node.from, to: node.to, deco: syntaxHide });
      }
    },
  });

  decorations.sort(
    (a, b) => a.from - b.from || a.deco.startSide - b.deco.startSide || a.to - b.to,
  );
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

export interface LivePreviewOptions {
  resolveImageSrc?: ImageSrcResolver;
}

export function markdownLivePreview(options?: LivePreviewOptions) {
  const extensions = [livePreviewPlugin];
  if (options?.resolveImageSrc) {
    extensions.unshift(imageResolverFacet.of(options.resolveImageSrc));
  }
  return extensions;
}
```

**Step 4: Update the existing test helper**

The existing `createView` function uses `markdownLivePreview()` with no args — this still works since the options parameter is optional. No change needed to existing tests.

**Step 5: Run tests to verify they pass**

Run: `npx vitest run src/lib/markdownLivePreview.test.ts`
Expected: All tests pass (existing + 3 new).

**Step 6: Commit**

```bash
git add src/lib/markdownLivePreview.ts src/lib/markdownLivePreview.test.ts
git commit -m "feat(notes): render images inline in markdown live preview"
```

---

### Task 4: Paste and drop image handlers

**Files:**
- Modify: `src/lib/CodeMirrorNoteEditor.svelte`

**Step 1: Add new props to the interface**

In `src/lib/CodeMirrorNoteEditor.svelte`, update the `Props` interface (around line 37):

```typescript
  interface Props {
    value: string;
    focused?: boolean;
    entryKey?: string;
    projectName?: string;
    resolveImageSrc?: (path: string) => string | null;
    onChange?: (value: string) => void;
    onEscape?: (mode: VimMode | string) => void;
    onModeChange?: (mode: VimMode | string) => void;
    onAiChat?: (request: AiChatRequest) => void;
    onImageSaved?: (relativePath: string) => void;
  }
```

Update the destructuring (line 47):

```typescript
  let { value, focused = false, entryKey, projectName, resolveImageSrc, onChange, onEscape, onModeChange, onAiChat, onImageSaved }: Props = $props();
```

**Step 2: Update markdownLivePreview call**

Update the `buildState` function to pass the resolver. Change line 60 from:

```typescript
        markdownLivePreview(),
```

to:

```typescript
        markdownLivePreview({ resolveImageSrc: untrack(() => resolveImageSrc) }),
```

**Step 3: Add paste and drop image handler helper**

Add this helper function before `buildState` (after the `$props()` line):

```typescript
  const ALLOWED_IMAGE_TYPES = ["image/png", "image/jpeg", "image/gif", "image/webp"];

  function extensionFromMime(mime: string): string | null {
    const map: Record<string, string> = {
      "image/png": "png",
      "image/jpeg": "jpg",
      "image/gif": "gif",
      "image/webp": "webp",
    };
    return map[mime] ?? null;
  }

  async function handleImageFile(file: File, insertPos: number) {
    if (!projectName) return;
    const ext = extensionFromMime(file.type);
    if (!ext) return;

    const { command } = await import("$lib/backend");
    const arrayBuf = await file.arrayBuffer();
    const bytes = Array.from(new Uint8Array(arrayBuf));

    const relativePath = await command<string>("save_note_image", {
      projectName,
      imageBytes: bytes,
      extension: ext,
    });

    if (!view) return;

    const mdText = `![](${relativePath})`;
    view.dispatch({
      changes: { from: insertPos, to: insertPos, insert: mdText },
    });
    onChange?.(view.state.doc.toString());
    onImageSaved?.(relativePath);
  }
```

**Step 4: Add paste and drop event handlers**

In the `buildState` function, add paste and drop handlers to the existing `EditorView.domEventHandlers`. Change the handler from:

```typescript
        EditorView.domEventHandlers({
          keydown: (event) => {
            if (event.key === "Escape") {
              onEscape?.(currentMode);
            }
            return false;
          },
        }),
```

to:

```typescript
        EditorView.domEventHandlers({
          keydown: (event) => {
            if (event.key === "Escape") {
              onEscape?.(currentMode);
            }
            return false;
          },
          paste: (event, editorView) => {
            const files = event.clipboardData?.files;
            if (!files || files.length === 0) return false;
            for (const file of files) {
              if (ALLOWED_IMAGE_TYPES.includes(file.type)) {
                event.preventDefault();
                const pos = editorView.state.selection.main.head;
                handleImageFile(file, pos);
                return true;
              }
            }
            return false;
          },
          drop: (event, editorView) => {
            const files = event.dataTransfer?.files;
            if (!files || files.length === 0) return false;
            for (const file of files) {
              if (ALLOWED_IMAGE_TYPES.includes(file.type)) {
                event.preventDefault();
                const dropPos = editorView.posAtCoords({
                  x: event.clientX,
                  y: event.clientY,
                });
                const pos = dropPos ?? editorView.state.selection.main.head;
                handleImageFile(file, pos);
                return true;
              }
            }
            return false;
          },
        }),
```

**Step 5: Verify it compiles**

Run: `npx vitest run src/lib/markdownLivePreview.test.ts`
Expected: Still passes (no breaking changes to existing tests).

**Step 6: Commit**

```bash
git add src/lib/CodeMirrorNoteEditor.svelte
git commit -m "feat(notes): add paste and drop image handlers to editor"
```

---

### Task 5: Wire up NotesEditor with image resolution

**Files:**
- Modify: `src/lib/NotesEditor.svelte`

**Step 1: Add image resolution state and callback**

In `src/lib/NotesEditor.svelte`, add these after the existing state declarations (after line 14):

```typescript
  let assetUrlCache = $state(new Map<string, string>());
```

Add this function after the `handleEditorChange` function (after line 120):

```typescript
  async function resolveImageSrc(relativePath: string): string | null {
    if (relativePath.startsWith("http://") || relativePath.startsWith("https://")) {
      return relativePath;
    }
    const cached = assetUrlCache.get(relativePath);
    if (cached) return cached;

    if (!projectName) return null;

    try {
      const absPath = await command<string>("resolve_note_asset_path", {
        projectName,
        relativePath,
      });
      const { convertFileSrc } = await import("@tauri-apps/api/core");
      const url = convertFileSrc(absPath);
      assetUrlCache.set(relativePath, url);
      return url;
    } catch {
      return null;
    }
  }

  // Synchronous resolver that returns from cache or triggers async resolution
  function resolveImageSrcSync(relativePath: string): string | null {
    if (relativePath.startsWith("http://") || relativePath.startsWith("https://")) {
      return relativePath;
    }
    const cached = assetUrlCache.get(relativePath);
    if (cached) return cached;
    // Trigger async resolution — the image will appear on next decoration rebuild
    resolveImageSrc(relativePath);
    return null;
  }

  function handleImageSaved(relativePath: string) {
    // Pre-warm the cache so the image renders immediately
    resolveImageSrc(relativePath);
  }
```

**Step 2: Clear cache when switching notes**

Inside the `$effect` that handles note switching (the `if (key !== prev)` block, around line 47), add cache clearing after `prevNoteKey = key;`:

```typescript
      assetUrlCache = new Map();
```

**Step 3: Pass new props to CodeMirrorNoteEditor**

Update the `<CodeMirrorNoteEditor>` element (around line 156) to pass the new props:

```svelte
      <CodeMirrorNoteEditor
        value={content}
        focused={editorFocused}
        entryKey={editorEntryKey}
        projectName={projectName ?? undefined}
        resolveImageSrc={resolveImageSrcSync}
        onChange={handleEditorChange}
        onModeChange={(mode) => {
          editorMode = mode;
        }}
        onEscape={handleEditorEscape}
        onAiChat={(request) => {
          aiChatRequest = request;
        }}
        onImageSaved={handleImageSaved}
      />
```

**Step 4: Verify compilation**

Run: `npx vitest run`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add src/lib/NotesEditor.svelte
git commit -m "feat(notes): wire up image resolution and cache in NotesEditor"
```

---

### Task 6: Image styles

**Files:**
- Modify: `src/lib/CodeMirrorNoteEditor.svelte` (style block)

**Step 1: Add image styles**

In the `<style>` block of `src/lib/CodeMirrorNoteEditor.svelte`, add after the `.cm-md-codeblock-line` rule (after line 285):

```css
  /* Images */
  .note-code-editor :global(.cm-md-image) {
    padding: 4px 0;
  }

  .note-code-editor :global(.cm-md-image img) {
    max-width: 100%;
    height: auto;
    border-radius: 4px;
    display: block;
  }
```

**Step 2: Commit**

```bash
git add src/lib/CodeMirrorNoteEditor.svelte
git commit -m "feat(notes): add image preview styles"
```

---

### Task 7: AI prompt update

**Files:**
- Modify: `src-tauri/src/note_ai_chat.rs:25-36`

**Step 1: Write failing test**

Add this test in the `mod tests` block of `src-tauri/src/note_ai_chat.rs`:

```rust
    #[test]
    fn build_prompt_mentions_image_syntax() {
        let prompt = build_note_ai_prompt("note", "selected", &[], "help");
        assert!(prompt.contains("!["), "prompt should mention image markdown syntax");
    }
```

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test note_ai_chat::tests::build_prompt_mentions_image_syntax`
Expected: FAIL — the prompt doesn't contain `![`.

**Step 3: Update the system prompt**

In `src-tauri/src/note_ai_chat.rs`, update the system prompt string (lines 24-36). Add this line after the `"Use \"info\" when..."` line:

```
        \n\
        The note supports markdown image syntax: ![description](url) for images.\n\
        You can include images using URLs when relevant to the user's request.\n\
```

The full updated string (lines 24-37) becomes:

```rust
    parts.push(
        "You are a note-editing AI assistant. The user has selected text in a note and is asking you to help with it.\n\
        \n\
        You MUST return ONLY valid JSON with one of these shapes:\n\
        {\"type\":\"replace\",\"text\":\"the new text that will replace the selection\"}\n\
        {\"type\":\"info\",\"text\":\"an informational response that does not modify the note\"}\n\
        \n\
        Use \"replace\" when the user wants to modify, rewrite, fix, or transform the selected text.\n\
        Use \"info\" when the user is asking a question about the text or wants an explanation without changes.\n\
        \n\
        The note supports markdown image syntax: ![description](url) for images.\n\
        You can include images using URLs when relevant to the user's request.\n\
        \n\
        If the user asks you to revert, return a \"replace\" with the original selected text.\n\
        \n\
        Do NOT wrap JSON in markdown code fences. Return raw JSON only.".to_string(),
    );
```

**Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test note_ai_chat::tests`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add src-tauri/src/note_ai_chat.rs
git commit -m "feat(notes): mention image syntax in AI assistant prompt"
```

---

### Task 8: Final verification

**Step 1: Run all Rust tests**

Run: `cd src-tauri && cargo test`
Expected: All tests pass.

**Step 2: Run all frontend tests**

Run: `npx vitest run`
Expected: All tests pass.

**Step 3: Verify compilation**

Run: `cd src-tauri && cargo check`
Expected: No errors.

**Step 4: Final commit if any cleanup needed**

If no cleanup needed, this task is complete.
