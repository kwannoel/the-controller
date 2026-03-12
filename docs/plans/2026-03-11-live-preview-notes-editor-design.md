# Live Preview Notes Editor

## Goal

Replace the separate edit/preview/split modes with a single Notion-like editor that renders markdown inline while supporting full vim keybindings. No mode buttons — vim is the interface.

## Approach: CodeMirror Decorations (ViewPlugin)

Keep CodeMirror 6 + `@replit/codemirror-vim`. Add a `ViewPlugin` that reads the markdown syntax tree and produces decorations to hide syntax markers and style content inline.

### New file: `src/lib/markdownLivePreview.ts`

A CodeMirror `ViewPlugin` returning a `DecorationSet`. Two decoration types:

- **`Decoration.replace()`** — hides syntax markers (`#`, `**`, `*`, backticks, `- `, `[`, `](url)`, fence lines)
- **`Decoration.mark()`** — applies CSS classes for visual styling (heading sizes, bold, italic, code background, etc.)

**Cursor-aware rule:** When the cursor is on a markdown node's line, decorations for that line are removed, revealing raw syntax for editing.

### Elements

| Element | Hide | Style |
|---------|------|-------|
| `# H1` | `# ` | 24px, weight 700, bottom border |
| `## H2` | `## ` | 20px, weight 600 |
| `### H3` | `### ` | 16px, weight 600 |
| `**bold**` | `**` both sides | font-weight 700 |
| `*italic*` | `*` both sides | font-style italic |
| `` `code` `` | backticks | bg surface, mono, rounded |
| `- list item` | `- ` | bullet + indent |
| `[text](url)` | `[`, `](url)` | white, underline on hover |
| Code blocks | fence lines | bg surface, mono, padding, rounded |

### Integration changes

**`CodeMirrorNoteEditor.svelte`:**
- Import and add the live preview plugin to extensions array
- Add CSS classes for decoration styling

**`NotesEditor.svelte`:**
- Remove Edit/Preview/Split toggle buttons and mode state
- Remove preview pane rendering
- Editor always shown full-width

### Unchanged

- Vim keybindings, mode tracking, `ga` AI chat
- Save logic (debounced, on escape)
- WKWebView key event patches
- Focus management
- `noteViewMode` store and `markdown.ts` remain in codebase (may be used elsewhere)
