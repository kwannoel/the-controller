# Drag and Drop Images Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enable dragging image files from Finder into Claude terminal sessions, copying to clipboard and triggering Claude Code's image reader.

**Architecture:** Listen for Tauri's `tauri://drag-drop` event (which provides file paths), add a Rust command that reads the image file and copies it to the system clipboard, then send the bracket-paste sequence to trigger Claude Code's clipboard reader.

**Tech Stack:** Svelte 5, TypeScript, Tauri v2 events, `image` crate (Rust), `tauri-plugin-clipboard-manager`

---

### Task 1: Add `image` crate dependency and `write-image` permission

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/capabilities/default.json`

**Step 1: Add the image crate**

Add to `[dependencies]` in `src-tauri/Cargo.toml`:

```toml
image = { version = "0.25", default-features = false, features = ["png", "jpeg", "gif", "webp", "bmp"] }
```

**Step 2: Add clipboard write-image permission**

Update `src-tauri/capabilities/default.json` to add `"clipboard-manager:allow-write-image"`:

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
    "clipboard-manager:allow-read-image",
    "clipboard-manager:allow-write-image"
  ]
}
```

**Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | tail -5`
Expected: compiles without errors

**Step 4: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/capabilities/default.json
git commit -m "feat: add image crate and clipboard write permission for drag-drop (#34)"
```

---

### Task 2: Add `copy_image_file_to_clipboard` Rust command

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: Add the command to `commands.rs`**

Add this function at the end of `src-tauri/src/commands.rs`:

```rust
/// Read an image file from disk and copy it to the system clipboard.
/// Used by the drag-and-drop handler to put the dropped image on the clipboard
/// so Claude Code can read it via its standard clipboard image detection.
#[tauri::command]
pub async fn copy_image_file_to_clipboard(app: AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;

    let image_data = tokio::task::spawn_blocking(move || {
        let img = image::open(&path).map_err(|e| format!("Failed to open image: {e}"))?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        Ok::<_, String>((rgba.into_raw(), width, height))
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))??;

    let (bytes, width, height) = image_data;
    let img = tauri::image::Image::new_owned(bytes, width, height);
    app.clipboard()
        .write_image(&img)
        .map_err(|e| format!("Failed to write image to clipboard: {e}"))
}
```

Add `use tauri::AppHandle;` to the imports at the top of `commands.rs` if not already present. The existing imports already have `AppHandle` via `use tauri::{AppHandle, Emitter, State};`.

**Step 2: Register the command in `lib.rs`**

Add `commands::copy_image_file_to_clipboard` to the `invoke_handler` list in `src-tauri/src/lib.rs`:

```rust
commands::merge_session_branch,
commands::copy_image_file_to_clipboard,
```

**Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | tail -5`
Expected: compiles without errors

**Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add copy_image_file_to_clipboard command (#34)"
```

---

### Task 3: Add drag-drop handler to Terminal.svelte

**Files:**
- Modify: `src/lib/Terminal.svelte`

**Step 1: Add the drag-drop event listener**

In `Terminal.svelte`, add the import for `listen` (already imported) and add the drag-drop handler. The handler listens for `tauri://drag-drop`, filters for image files, copies the first image to clipboard, and sends the bracket-paste sequence.

Add a new variable for the unlisten function:

```typescript
let unlistenDragDrop: UnlistenFn | undefined;
```

Add image extension check helper before `onMount`:

```typescript
const IMAGE_EXTENSIONS = new Set([
  ".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp",
]);

function isImageFile(path: string): boolean {
  const ext = path.slice(path.lastIndexOf(".")).toLowerCase();
  return IMAGE_EXTENSIONS.has(ext);
}
```

Inside `onMount`, after the existing `listen` calls, add:

```typescript
// Listen for drag-and-drop file events (from Finder)
listen<{ paths: string[] }>("tauri://drag-drop", async (event) => {
  const imagePath = event.payload.paths.find(isImageFile);
  if (imagePath) {
    try {
      await invoke("copy_image_file_to_clipboard", { path: imagePath });
      await writeToPty("\x1b[200~\x1b[201~");
    } catch (err) {
      console.error("Failed to handle dropped image:", err);
    }
  }
}).then((fn) => {
  unlistenDragDrop = fn;
});
```

In `onDestroy`, add the cleanup:

```typescript
unlistenDragDrop?.();
```

**Step 2: Verify the frontend builds**

Run: `npm run build 2>&1 | tail -10`
Expected: Build succeeds

**Step 3: Commit**

```bash
git add src/lib/Terminal.svelte
git commit -m "feat: handle drag-and-drop images in terminal (#34)"
```

---

### Task 4: Manual testing

**Step 1: Start the dev server**

Run: `npm run tauri dev`

**Step 2: Test image drag-and-drop**

1. Open a Claude session in the controller
2. Drag a PNG/JPG file from Finder into the terminal area
3. Expected: Claude Code should detect the image from clipboard and offer to use it

**Step 3: Test non-image drag-and-drop**

1. Drag a `.txt` or `.rs` file from Finder into the terminal area
2. Expected: Nothing happens (no crash, no error)

**Step 4: Test that Cmd+V paste still works**

1. Copy an image to clipboard (screenshot with Cmd+Shift+4)
2. Press Cmd+V in the terminal
3. Expected: Claude Code detects the image as before

---

### Task 5: Final cleanup and PR

**Step 1: Run all tests**

Run: `npm test`
Expected: All tests pass

**Step 2: Run type checking**

Run: `npm run check`
Expected: No type errors

**Step 3: Create PR**

```bash
gh pr create --title "feat: support drag and drop of images into terminal" --body "$(cat <<'EOF'
## Summary
- Add `copy_image_file_to_clipboard` Rust command that reads an image file and copies it to the system clipboard
- Listen for `tauri://drag-drop` events in Terminal.svelte to handle files dragged from Finder
- When an image file is dropped, copy it to clipboard and send bracket-paste sequence to trigger Claude Code's image reader

closes #34

## Test plan
- [ ] Drag PNG/JPG/GIF/WebP from Finder into terminal — Claude Code detects image
- [ ] Drag non-image file — nothing happens
- [ ] Cmd+V paste still works for clipboard images
- [ ] Cmd+V paste still works for text

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```
