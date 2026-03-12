# Notes Image Support Design

## Goal

Support images in notes mode — paste, drag & drop, manual markdown, and AI-inserted images rendered inline via the Notion-like live preview.

## Decisions

- **Storage:** `~/.the-controller/notes/{project}/assets/{uuid8}.{ext}` — project-scoped, auto-generated filenames
- **Formats:** PNG, JPG, JPEG, GIF, WebP only
- **Markdown syntax:** `![alt](assets/filename.png)` for local, `![alt](https://...)` for external
- **Rendering:** `max-width: 100%` inline images in live preview, raw markdown shown when cursor is on the line
- **No optimization:** Store images as-is, no resizing or compression
- **AI:** Update system prompt so AI knows it can use `![](url)` syntax; no special UI needed

## Architecture

### Image Storage (Rust)

New functions in `src-tauri/src/notes.rs`:

- `save_note_image(base, project_name, image_bytes, extension) -> String`
  - Validates extension is one of: png, jpg, jpeg, gif, webp
  - Creates `{notes_dir}/assets/` if needed
  - Writes to `assets/{uuid8}.{ext}`
  - Returns relative path `assets/{uuid8}.{ext}`

- `resolve_note_asset_path(base, project_name, relative_path) -> PathBuf`
  - Resolves `assets/foo.png` to absolute filesystem path
  - Validates path doesn't escape notes directory (no `..`)

New Tauri commands in `src-tauri/src/commands/notes.rs`, registered in `lib.rs`.

### Live Preview (TypeScript)

`markdownLivePreview.ts` changes:

- Accept `resolveImageSrc: (path: string) => string | null` parameter
- Handle `Image` syntax tree node: when cursor is not on the line, replace `![alt](path)` with an `ImageWidget` decoration
- `ImageWidget extends WidgetType` renders `<img>` with resolved src
- External URLs (http/https) passed through directly
- Local relative paths resolved via the callback

### Paste & Drop (Svelte)

`CodeMirrorNoteEditor.svelte` changes:

- New props: `projectName: string`, `onImageSaved?: (relativePath: string) => void`
- `EditorView.domEventHandlers` for `paste`: check `clipboardData.files` for image MIME types, save via Tauri command, insert `![](assets/{name})` at cursor
- `EditorView.domEventHandlers` for `drop`: same flow using `dataTransfer.files`, insert at drop position

`NotesEditor.svelte` changes:

- Maintain asset URL cache (Map<string, string>) mapping relative paths to Tauri asset URLs via `convertFileSrc`
- Pass `projectName` and `resolveImageSrc` callback to editor
- On `onImageSaved`, add to cache

### AI Chat

`note_ai_chat.rs`: Add to the system prompt that `![description](url)` syntax is supported for images.

## Files Changed

| File | Change |
|------|--------|
| `src-tauri/src/notes.rs` | `save_note_image()`, `resolve_note_asset_path()` + tests |
| `src-tauri/src/commands/notes.rs` | New command handlers |
| `src-tauri/src/lib.rs` | Register commands |
| `src/lib/markdownLivePreview.ts` | `Image` node handler, `ImageWidget`, `resolveImageSrc` param |
| `src/lib/CodeMirrorNoteEditor.svelte` | Paste/drop handlers, new props, styles |
| `src/lib/NotesEditor.svelte` | Asset URL cache, prop plumbing |
| `src-tauri/src/note_ai_chat.rs` | Update system prompt |
| Tests | Rust unit tests, TS decoration tests |

## Out of Scope

- Image optimization/resizing
- Data URL embedding
- Image deletion/cleanup
- Drag reordering
- SVG support
