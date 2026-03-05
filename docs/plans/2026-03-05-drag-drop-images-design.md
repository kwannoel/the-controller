# Drag and Drop Images Design

## Problem
Users cannot drag and drop image files from Finder into Claude terminal sessions. Drag & drop is a separate browser/Tauri event from clipboard paste and needs its own handling.

## Approach
Copy-to-clipboard + bracket paste: On drop, read the image file, copy it to the system clipboard via Tauri's clipboard plugin, then send the same empty bracket-paste sequence (`\x1b[200~\x1b[201~`) that Cmd+V uses. This reuses the existing Claude Code clipboard-reading path.

## Components
- `src-tauri/src/commands.rs` — new `copy_image_file_to_clipboard` command
- `src/lib/Terminal.svelte` — `dragover`/`drop` event handlers
- `src-tauri/capabilities/default.json` — add `clipboard-manager:allow-write-image`
- `src-tauri/Cargo.toml` — add `image` crate for decoding

## Key Details
- Tauri clipboard plugin's `write_image` needs RGBA bytes + dimensions, so we decode the file with the `image` crate on the Rust side
- Only handle image file extensions: png, jpg, jpeg, gif, webp, bmp
- Drop handler uses browser `dragover`/`drop` events on the terminal container div — Tauri's file drop gives us the file path
- Single image only (first image file if multiple dropped)
