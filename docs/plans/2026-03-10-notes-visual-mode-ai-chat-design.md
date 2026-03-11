# Notes Visual Mode AI Chat

## Summary

Augment the notes editor's Vim visual mode with an inline AI chat panel. After highlighting text in visual mode, pressing `ga` opens a floating panel near the selection where the user can freeform-prompt the AI about or transform the selected text.

## Decisions

| Aspect | Decision |
|--------|----------|
| Trigger | `ga` in visual mode |
| UI | Floating panel anchored near selection |
| Backend | `codex exec` subprocess (stateless, like controller chat) |
| Multi-turn | Frontend manages conversation history |
| Transformations | Immediate replacement in editor, undo with `u` |
| Revert via AI | User says "revert that" in conversation |
| Response format | JSON `{ type: "replace" \| "info", text }` |
| Dismissal | Escape closes panel, returns to normal mode |

## Interaction Flow

1. User enters visual mode, highlights text
2. Presses `ga` — floating panel appears anchored near the selection
3. Panel shows the selected text (dimmed/quoted) and a text input
4. User types a freeform prompt and hits Enter
5. AI receives: full note content + selected text range + user prompt
6. Response: `replace` swaps the selected text in the editor immediately; `info` displays in the panel
7. Panel stays open for follow-ups (multi-turn)
8. User can ask AI to "revert that" — AI re-emits previous version as a `replace`
9. `u` in normal mode also undoes any transformation
10. Escape dismisses the panel

## Floating Panel UI

- **Position:** Anchored below the selection (or above if near bottom). Uses CodeMirror `view.coordsAtPos()` for pixel coordinates
- **Width:** ~400px, max-height ~300px with scroll
- **Contents:**
  - Selected text shown as dimmed blockquote (collapsed if long)
  - Response area (markdown-rendered)
  - Text input at bottom: "Ask about selection..."
  - Spinner while waiting for response
- **Dismissal:** Escape
- **Styling:** Catppuccin Mocha, subtle border/shadow to float over editor

## Backend: `note_ai_chat` Tauri Command

**Input:**
```typescript
{
  noteContent: string,
  selectedText: string,
  selectionRange: { from: number, to: number },
  conversationHistory: Array<{ role: "user" | "assistant", content: string }>,
  prompt: string
}
```

**Behavior:**
- Builds system prompt with full note content and selected text context
- Appends conversation history
- Runs `codex exec` (same pattern as `controller_chat.rs`)
- Parses JSON response

**Output:**
```typescript
{ type: "replace", text: string } | { type: "info", text: string }
```

**System prompt instructs the AI to:**
- Consider the full note for context
- Focus on the selected text
- Return `replace` when user asks for a transformation
- Return `info` when user asks a question
- When asked to revert, return previous version as `replace`

## Keybinding: `ga` in Visual Mode

- Registered via `Vim.defineAction()` + `Vim.mapCommand()` on the CodeMirror vim instance
- Extracts selection range and text from `view.state.selection.main`
- Emits `onAiChat({ selectedText, from, to })` callback to parent
- Editor returns to normal mode but selection highlight preserved via CodeMirror decoration

## Component Structure

```
NotesEditor.svelte
├── CodeMirrorNoteEditor.svelte  (adds ga keybinding, emits onAiChat)
└── NoteAiPanel.svelte (new)     (floating panel, manages conversation, calls note_ai_chat)
```

## Validation

Run multiple end-to-end scenarios with Playwright (debugging-ui-with-playwright skill):
- Highlight single line, ask a question (info response)
- Highlight multiline, ask for summarization (replace response)
- Follow up with "revert that" (replace with original)
- Follow up with further transformation
- Dismiss with Escape
- Undo transformation with `u` in normal mode
