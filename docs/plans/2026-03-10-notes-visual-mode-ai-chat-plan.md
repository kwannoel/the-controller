# Notes Visual Mode AI Chat — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Add inline AI chat to the notes editor's Vim visual mode — `ga` on a selection opens a floating panel for freeform AI prompts that can transform the selected text.

**Architecture:** A new `note_ai_chat` Rust module handles AI calls via `codex exec` (same pattern as `controller_chat.rs`). The CodeMirror editor registers `ga` as a custom Vim action that emits selection data to the parent. A new `NoteAiPanel.svelte` floating component manages the conversation UI, positioned near the selection using CodeMirror's coordinate API.

**Tech Stack:** Svelte 5 (runes), CodeMirror 6, `@replit/codemirror-vim`, Rust/Tauri, `codex exec`

---

### Task 1: Backend — `note_ai_chat` Rust module

**Files:**
- Create: `src-tauri/src/note_ai_chat.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod note_ai_chat;`)

**Step 1: Write failing test for prompt building and response parsing**

In `src-tauri/src/note_ai_chat.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteAiChatMessage {
    pub role: String,   // "user" or "assistant"
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NoteAiResponse {
    Replace { text: String },
    Info { text: String },
}

fn build_note_ai_prompt(
    note_content: &str,
    selected_text: &str,
    conversation_history: &[NoteAiChatMessage],
    prompt: &str,
) -> String {
    let history_json = serde_json::to_string(conversation_history)
        .unwrap_or_else(|_| "[]".to_string());

    format!(
        "You are an AI assistant helping edit a markdown note.\n\
Return ONLY valid JSON with one of these shapes:\n\
{{\"type\":\"replace\",\"text\":\"...\"}}\n\
{{\"type\":\"info\",\"text\":\"...\"}}\n\n\
Use \"replace\" when the user asks you to transform, rewrite, fix, or change the selected text. \
The \"text\" field must contain the full replacement text.\n\
Use \"info\" when the user asks a question or wants an explanation. \
The \"text\" field contains your answer.\n\
When asked to revert, return the previous version of the selected text as a \"replace\".\n\n\
--- FULL NOTE ---\n{note_content}\n--- END NOTE ---\n\n\
--- SELECTED TEXT ---\n{selected_text}\n--- END SELECTED TEXT ---\n\n\
Conversation so far:\n{history_json}\n\n\
User prompt: {prompt}"
    )
}

pub fn parse_note_ai_response(raw: &str) -> Result<NoteAiResponse, String> {
    serde_json::from_str(raw)
        .map_err(|e| format!("Failed to parse note AI response: {}", e))
}

fn run_note_ai_turn(prompt: String) -> Result<NoteAiResponse, String> {
    let output = std::process::Command::new("codex")
        .arg("exec")
        .arg("--sandbox")
        .arg("danger-full-access")
        .arg(&prompt)
        .env_remove("CLAUDECODE")
        .output()
        .map_err(|e| format!("Failed to run codex exec: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("codex exec failed: {}", stderr.trim()));
    }

    parse_note_ai_response(String::from_utf8_lossy(&output.stdout).trim())
}

pub async fn send_note_ai_message(
    note_content: String,
    selected_text: String,
    conversation_history: Vec<NoteAiChatMessage>,
    prompt: String,
) -> Result<NoteAiResponse, String> {
    let built_prompt = build_note_ai_prompt(
        &note_content,
        &selected_text,
        &conversation_history,
        &prompt,
    );

    tokio::task::spawn_blocking(move || run_note_ai_turn(built_prompt))
        .await
        .map_err(|e| format!("Task failed: {}", e))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_replace_response() {
        let raw = r#"{"type":"replace","text":"new content"}"#;
        let response = parse_note_ai_response(raw).unwrap();
        match response {
            NoteAiResponse::Replace { text } => assert_eq!(text, "new content"),
            _ => panic!("expected Replace"),
        }
    }

    #[test]
    fn parse_info_response() {
        let raw = r#"{"type":"info","text":"This is an explanation."}"#;
        let response = parse_note_ai_response(raw).unwrap();
        match response {
            NoteAiResponse::Info { text } => assert_eq!(text, "This is an explanation."),
            _ => panic!("expected Info"),
        }
    }

    #[test]
    fn parse_invalid_response() {
        let raw = "not json";
        assert!(parse_note_ai_response(raw).is_err());
    }

    #[test]
    fn build_prompt_includes_note_and_selection() {
        let prompt = build_note_ai_prompt(
            "# My Note\nSome content here.",
            "Some content",
            &[],
            "summarize this",
        );
        assert!(prompt.contains("# My Note"));
        assert!(prompt.contains("Some content"));
        assert!(prompt.contains("summarize this"));
        assert!(prompt.contains("SELECTED TEXT"));
        assert!(prompt.contains("FULL NOTE"));
    }

    #[test]
    fn build_prompt_includes_conversation_history() {
        let history = vec![
            NoteAiChatMessage {
                role: "user".to_string(),
                content: "rewrite this".to_string(),
            },
            NoteAiChatMessage {
                role: "assistant".to_string(),
                content: "done".to_string(),
            },
        ];
        let prompt = build_note_ai_prompt("note", "selected", &history, "now shorten it");
        assert!(prompt.contains("rewrite this"));
        assert!(prompt.contains("now shorten it"));
    }
}
```

**Step 2: Add module declaration**

In `src-tauri/src/lib.rs`, add after the existing `mod` declarations:

```rust
mod note_ai_chat;
```

**Step 3: Run tests to verify they pass**

Run: `cd src-tauri && cargo test note_ai_chat`
Expected: All 4 tests pass

**Step 4: Commit**

```bash
git add src-tauri/src/note_ai_chat.rs src-tauri/src/lib.rs
git commit -m "feat: add note_ai_chat backend module with codex exec"
```

---

### Task 2: Backend — Tauri command for `note_ai_chat`

**Files:**
- Create: `src-tauri/src/commands/note_ai_chat.rs`
- Modify: `src-tauri/src/commands.rs` (add `mod note_ai_chat;` and re-export)
- Modify: `src-tauri/src/lib.rs` (register in `generate_handler!`)

**Step 1: Create the Tauri command**

In `src-tauri/src/commands/note_ai_chat.rs`:

```rust
use crate::note_ai_chat::{self, NoteAiChatMessage, NoteAiResponse};

#[derive(serde::Deserialize)]
pub struct SelectionRange {
    pub from: usize,
    pub to: usize,
}

pub async fn note_ai_chat(
    note_content: String,
    selected_text: String,
    selection_range: SelectionRange,
    conversation_history: Vec<NoteAiChatMessage>,
    prompt: String,
) -> Result<NoteAiResponse, String> {
    note_ai_chat::send_note_ai_message(
        note_content,
        selected_text,
        conversation_history,
        prompt,
    )
    .await
}
```

**Step 2: Wire up the command**

In `src-tauri/src/commands.rs`, add the module declaration near the top with the other `mod` statements:

```rust
mod note_ai_chat;
```

In `src-tauri/src/lib.rs`, add to the `generate_handler!` list (after `send_controller_chat_message`):

```rust
commands::note_ai_chat,
```

Note: The command function in `commands/note_ai_chat.rs` needs the `#[tauri::command]` attribute. Also, the `note_ai_chat` module name at the command level needs to match `commands::note_ai_chat` — since there's a conflict with the module name, rename the command function to `send_note_ai_chat`:

```rust
#[tauri::command]
pub async fn send_note_ai_chat(
    note_content: String,
    selected_text: String,
    selection_range: SelectionRange,
    conversation_history: Vec<NoteAiChatMessage>,
    prompt: String,
) -> Result<NoteAiResponse, String> {
    crate::note_ai_chat::send_note_ai_message(
        note_content,
        selected_text,
        conversation_history,
        prompt,
    )
    .await
}
```

And register as `commands::send_note_ai_chat` in `lib.rs`.

**Step 3: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add src-tauri/src/commands/note_ai_chat.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add send_note_ai_chat Tauri command"
```

---

### Task 3: Frontend — `ga` keybinding in CodeMirrorNoteEditor

**Files:**
- Modify: `src/lib/CodeMirrorNoteEditor.svelte`

**Context:** The editor uses `@replit/codemirror-vim`. To register a custom Vim action for visual mode, use `Vim.defineAction()` to create the action and `Vim.mapCommand()` to bind `ga` in visual mode. The action extracts the selection text and range, then calls an `onAiChat` callback prop.

**Step 1: Add the `onAiChat` callback prop**

In `CodeMirrorNoteEditor.svelte`, update the `Props` interface:

```typescript
export interface AiChatRequest {
  selectedText: string;
  from: number;
  to: number;
}
```

Add to `Props`:
```typescript
onAiChat?: (request: AiChatRequest) => void;
```

Add to the destructured props:
```typescript
let { value, focused = false, entryKey, onChange, onEscape, onModeChange, onAiChat }: Props = $props();
```

**Step 2: Register the `ga` Vim action**

Inside the `$effect` that creates the EditorView (after `const cm = getCM(view);`), register the action:

```typescript
Vim.defineAction("aiChat", (cm: any) => {
  if (!view) return;
  const sel = view.state.selection.main;
  if (sel.empty) return;
  const from = sel.from;
  const to = sel.to;
  const selectedText = view.state.doc.sliceString(from, to);
  onAiChat?.({ selectedText, from, to });
});
Vim.mapCommand("ga", "action", "aiChat", undefined, { context: "visual" });
```

Note: `Vim.defineAction` and `Vim.mapCommand` are called once during editor setup. The `context: "visual"` restricts the mapping to visual mode only.

**Step 3: Expose a method to get coordinates for positioning the panel**

Add a function and export it (or pass coords in the callback). Simpler: include coordinates in the `AiChatRequest`:

```typescript
export interface AiChatRequest {
  selectedText: string;
  from: number;
  to: number;
  coords: { left: number; top: number; bottom: number };
}
```

In the `aiChat` action, compute coords:
```typescript
const coords = view.coordsAtPos(from);
if (coords) {
  onAiChat?.({ selectedText, from, to, coords: { left: coords.left, top: coords.top, bottom: coords.bottom } });
}
```

**Step 4: Write a test for the ga action triggering onAiChat**

In `src/lib/NotesEditor.test.ts`, add a test:

```typescript
it("triggers onAiChat when ga is pressed in visual mode", async () => {
  // This is hard to unit test because Vim.defineAction requires a real CodeMirror
  // instance. The real validation is in the Playwright e2e test.
  // For now, verify the editor mounts and accepts the visual mode.
});
```

Actually, the real validation for `ga` will be via Playwright e2e tests (Task 6). The unit test coverage here is that the component accepts the new prop without breaking existing behavior.

**Step 5: Verify existing tests still pass**

Run: `npx vitest run src/lib/NotesEditor.test.ts`
Expected: All existing tests pass (the new optional prop doesn't break anything)

**Step 6: Commit**

```bash
git add src/lib/CodeMirrorNoteEditor.svelte
git commit -m "feat: register ga Vim action in visual mode for AI chat"
```

---

### Task 4: Frontend — `NoteAiPanel.svelte` floating panel component

**Files:**
- Create: `src/lib/NoteAiPanel.svelte`

**Context:** This is a floating panel that appears near the selection. It manages the conversation state, calls the `send_note_ai_chat` backend command, and emits replacement text back to the parent.

**Step 1: Create the component**

```svelte
<script lang="ts">
  import { command } from "$lib/backend";
  import { renderMarkdown } from "$lib/markdown";
  import type { AiChatRequest } from "./CodeMirrorNoteEditor.svelte";

  interface ConversationItem {
    role: "user" | "assistant";
    content: string;
    responseType?: "replace" | "info";
  }

  interface Props {
    noteContent: string;
    request: AiChatRequest;
    onReplace?: (text: string, from: number, to: number) => void;
    onDismiss?: () => void;
  }

  let { noteContent, request, onReplace, onDismiss }: Props = $props();

  let inputValue = $state("");
  let conversation = $state<ConversationItem[]>([]);
  let loading = $state(false);
  let scrollContainer: HTMLDivElement | undefined;
  let inputEl: HTMLInputElement | undefined;

  // Track the current selection range (may shift after replacements)
  let currentFrom = $state(request.from);
  let currentTo = $state(request.to);

  // Position the panel near the selection
  let panelStyle = $derived((() => {
    const { coords } = request;
    const left = Math.max(8, coords.left);
    const top = coords.bottom + 8;
    return `left: ${left}px; top: ${top}px;`;
  })());

  // Truncated preview of selected text
  let selectedPreview = $derived(
    request.selectedText.length > 200
      ? request.selectedText.slice(0, 200) + "..."
      : request.selectedText
  );

  $effect(() => {
    inputEl?.focus();
  });

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.stopPropagation();
      onDismiss?.();
    }
  }

  async function submit() {
    const prompt = inputValue.trim();
    if (!prompt || loading) return;

    conversation.push({ role: "user", content: prompt });
    inputValue = "";
    loading = true;

    try {
      const history = conversation
        .filter((item) => item.role === "user" || item.role === "assistant")
        .map((item) => ({ role: item.role, content: item.content }));

      const response = await command<{ type: string; text: string }>(
        "send_note_ai_chat",
        {
          noteContent,
          selectedText: request.selectedText,
          selectionRange: { from: currentFrom, to: currentTo },
          conversationHistory: history.slice(0, -1), // exclude current prompt
          prompt,
        }
      );

      conversation.push({
        role: "assistant",
        content: response.text,
        responseType: response.type as "replace" | "info",
      });

      if (response.type === "replace") {
        const newTo = currentFrom + response.text.length;
        onReplace?.(response.text, currentFrom, currentTo);
        currentTo = newTo;
      }
    } catch (error) {
      conversation.push({
        role: "assistant",
        content: `Error: ${error instanceof Error ? error.message : String(error)}`,
        responseType: "info",
      });
    } finally {
      loading = false;
      scrollToBottom();
    }
  }

  function scrollToBottom() {
    requestAnimationFrame(() => {
      if (scrollContainer) {
        scrollContainer.scrollTop = scrollContainer.scrollHeight;
      }
    });
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="note-ai-panel" style={panelStyle} data-testid="note-ai-panel" onkeydown={handleKeydown}>
  <div class="selected-preview">
    <pre>{selectedPreview}</pre>
  </div>

  <div class="conversation" bind:this={scrollContainer}>
    {#each conversation as item}
      <div class="message {item.role}">
        {#if item.role === "user"}
          <span class="label">You:</span> {item.content}
        {:else}
          <div class="ai-response">
            {#if item.responseType === "replace"}
              <span class="badge replace">replaced</span>
            {/if}
            {@html renderMarkdown(item.content)}
          </div>
        {/if}
      </div>
    {/each}
    {#if loading}
      <div class="message assistant">
        <span class="spinner"></span>
      </div>
    {/if}
  </div>

  <form class="input-row" onsubmit|preventDefault={submit}>
    <input
      bind:this={inputEl}
      bind:value={inputValue}
      placeholder="Ask about selection..."
      disabled={loading}
      data-testid="note-ai-input"
    />
  </form>
</div>

<style>
  .note-ai-panel {
    position: fixed;
    width: 400px;
    max-height: 340px;
    background: #1e1e2e;
    border: 1px solid #45475a;
    border-radius: 8px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    display: flex;
    flex-direction: column;
    z-index: 100;
    font-size: 13px;
    color: #cdd6f4;
  }

  .selected-preview {
    padding: 8px 12px;
    border-bottom: 1px solid #313244;
    max-height: 60px;
    overflow: hidden;
  }

  .selected-preview pre {
    margin: 0;
    font-size: 11px;
    color: #6c7086;
    white-space: pre-wrap;
    word-break: break-word;
    font-family: monospace;
  }

  .conversation {
    flex: 1;
    overflow-y: auto;
    padding: 8px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
  }

  .message {
    line-height: 1.5;
  }

  .message.user {
    color: #89b4fa;
  }

  .label {
    font-weight: 600;
  }

  .ai-response {
    color: #cdd6f4;
  }

  .badge {
    display: inline-block;
    font-size: 10px;
    padding: 1px 5px;
    border-radius: 3px;
    margin-bottom: 4px;
    font-weight: 600;
  }

  .badge.replace {
    background: #a6e3a1;
    color: #1e1e2e;
  }

  .spinner {
    display: inline-block;
    width: 12px;
    height: 12px;
    border: 2px solid #45475a;
    border-top-color: #89b4fa;
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .input-row {
    border-top: 1px solid #313244;
    padding: 8px;
  }

  .input-row input {
    width: 100%;
    background: #11111b;
    border: 1px solid #313244;
    border-radius: 4px;
    padding: 6px 10px;
    color: #cdd6f4;
    font-size: 13px;
    outline: none;
    box-sizing: border-box;
  }

  .input-row input:focus {
    border-color: #89b4fa;
  }

  .input-row input::placeholder {
    color: #6c7086;
  }
</style>
```

**Step 2: Verify it compiles**

Run: `npx vitest run` (quick check nothing is broken)
Expected: Existing tests still pass

**Step 3: Commit**

```bash
git add src/lib/NoteAiPanel.svelte
git commit -m "feat: add NoteAiPanel floating component for selection AI chat"
```

---

### Task 5: Frontend — Wire everything together in NotesEditor

**Files:**
- Modify: `src/lib/NotesEditor.svelte`

**Context:** `NotesEditor` is the parent that connects `CodeMirrorNoteEditor` (which emits `onAiChat`) with `NoteAiPanel` (which manages the conversation). When a replacement comes back, `NotesEditor` updates the editor content.

**Step 1: Add state and imports**

At the top of `NotesEditor.svelte`'s `<script>`, add:

```typescript
import NoteAiPanel from "./NoteAiPanel.svelte";
import type { AiChatRequest } from "./CodeMirrorNoteEditor.svelte";

let aiChatRequest = $state<AiChatRequest | null>(null);
```

**Step 2: Wire the `onAiChat` callback to CodeMirrorNoteEditor**

In the template where `CodeMirrorNoteEditor` is rendered, add the callback:

```svelte
<CodeMirrorNoteEditor
  value={content}
  focused={editorFocused}
  entryKey={editorEntryKey}
  onChange={handleEditorChange}
  onModeChange={(mode) => {
    editorMode = mode;
  }}
  onEscape={handleEditorEscape}
  onAiChat={(request) => {
    aiChatRequest = request;
  }}
/>
```

**Step 3: Render the floating panel conditionally**

After the `.editor-body` div, add:

```svelte
{#if aiChatRequest}
  <NoteAiPanel
    noteContent={content}
    request={aiChatRequest}
    onReplace={(text, from, to) => {
      content = content.slice(0, from) + text + content.slice(to);
      scheduleSave();
    }}
    onDismiss={() => {
      aiChatRequest = null;
    }}
  />
{/if}
```

**Step 4: Dismiss panel on Escape from editor**

In the existing `handleEditorEscape` function, also dismiss the AI panel:

```typescript
function handleEditorEscape(mode: VimMode | string) {
  editorMode = mode;
  if (aiChatRequest) {
    aiChatRequest = null;
    return;
  }
  if (editorMode !== "normal") return;
  // ... rest of existing logic
}
```

Wait — Escape in the AI panel is handled by the panel itself (via `onkeydown`). But if the user presses Escape while focus is in the editor (not the panel input), it should also dismiss. The panel's `onDismiss` handles this. Actually, since the panel captures focus in the input, Escape from the input calls `onDismiss`. If the user clicks back into the editor and presses Escape, the editor's `onEscape` fires. So we should dismiss the panel there too.

Actually, simpler: dismiss the panel when the editor's `onEscape` fires, regardless of mode, if the panel is open:

```typescript
function handleEditorEscape(mode: VimMode | string) {
  editorMode = mode;
  if (aiChatRequest) {
    aiChatRequest = null;
    return; // consume the escape
  }
  if (editorMode !== "normal") return;

  void saveNow();
  if (currentNote) {
    focusTarget.set({ type: "note", filename: currentNote.filename, projectId: currentNote.projectId });
  }
}
```

**Step 5: Verify existing tests still pass**

Run: `npx vitest run src/lib/NotesEditor.test.ts`
Expected: All existing tests pass

**Step 6: Commit**

```bash
git add src/lib/NotesEditor.svelte
git commit -m "feat: wire AI chat panel to notes editor with ga trigger"
```

---

### Task 6: End-to-end validation with Playwright

**Files:**
- Use the `debugging-ui-with-playwright` skill to run live scenarios

**Context:** This is manual end-to-end validation using Playwright in browser mode. The app must be running via `npm run tauri dev`.

**Scenarios to test:**

1. **Single-line selection + info response:**
   - Open a note, enter visual mode (`v`), select a word
   - Press `ga` — verify floating panel appears
   - Type "what does this mean?" — verify info response renders

2. **Multiline selection + replace response:**
   - Select multiple lines with `V` (visual line mode)
   - Press `ga`
   - Type "summarize this" — verify text gets replaced inline

3. **Revert via AI:**
   - After a replacement, type "revert that"
   - Verify original text is restored

4. **Multi-turn follow-up:**
   - After a transformation, type "now make it shorter"
   - Verify second transformation applies

5. **Dismiss with Escape:**
   - Open panel, press Escape
   - Verify panel closes

6. **Undo with `u`:**
   - After a replacement, press Escape to dismiss panel
   - Press `u` in normal mode
   - Verify editor content reverts

**Step 1: Run the app**

Run: `npm run tauri dev`

**Step 2: Use the `the-controller-debugging-ui-with-playwright` skill**

Run each scenario above. For each, verify the panel appears, responses render, replacements apply, and dismiss works.

**Step 3: Fix any issues found**

If a scenario fails, debug and fix before moving on.

**Step 4: Commit any fixes**

```bash
git add -A
git commit -m "fix: address issues found during e2e validation"
```
