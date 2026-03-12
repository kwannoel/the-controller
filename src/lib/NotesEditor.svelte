<script lang="ts">
  import { fromStore } from "svelte/store";
  import { untrack } from "svelte";
  import { command } from "$lib/backend";
  import { activeNote, noteViewMode, focusTarget, hotkeyAction, type NoteViewMode, type FocusTarget } from "./stores";
  import CodeMirrorNoteEditor, { type VimMode, type AiChatRequest } from "./CodeMirrorNoteEditor.svelte";
  import NoteAiPanel from "./NoteAiPanel.svelte";

  let content = $state("");
  let savedContent = $state("");
  let loading = $state(true);
  let saveTimer: ReturnType<typeof setTimeout> | null = $state(null);
  let editorMode = $state<VimMode | string>("normal");
  let aiChatRequest = $state<AiChatRequest | null>(null);

  const activeNoteState = fromStore(activeNote);
  let currentNote = $derived(activeNoteState.current);

  const viewModeState = fromStore(noteViewMode);
  let currentViewMode: NoteViewMode = $derived(viewModeState.current);
  let showsEditor = $derived(currentViewMode === "edit" || currentViewMode === "split");
  let showsPreview = $derived(currentViewMode === "preview" || currentViewMode === "split");

  const focusTargetState = fromStore(focusTarget);
  let currentFocus: FocusTarget = $derived(focusTargetState.current);

  let editorFocused = $derived(currentFocus?.type === "notes-editor");
  let editorEntryKey = $derived(currentFocus?.type === "notes-editor" ? currentFocus.entryKey : undefined);

  let folderName = $derived(currentNote?.folder ?? null);

  let noteTitle = $derived(
    currentNote
      ? currentNote.filename.replace(/\.md$/, "")
      : null
  );

  let isDirty = $derived(content !== savedContent);

  // Load note content when activeNote changes
  let prevNoteKey: string | null = $state(null);
  $effect(() => {
    const key = currentNote ? `${currentNote.folder}:${currentNote.filename}` : null;
    const prev = untrack(() => prevNoteKey);
    if (key !== prev) {
      // Flush unsaved content for the previous note before switching
      const timer = untrack(() => saveTimer);
      if (timer) {
        clearTimeout(timer);
        saveTimer = null;
        const prevContent = untrack(() => content);
        const prevSaved = untrack(() => savedContent);
        if (prevContent !== prevSaved && prev) {
          const [prevFolder, ...rest] = prev.split(":");
          const prevFilename = rest.join(":");
          if (prevFolder && prevFilename) {
            command("write_note", { folder: prevFolder, filename: prevFilename, content: prevContent }).catch(() => {});
          }
        }
      }
      prevNoteKey = key;

      if (currentNote && folderName && key) {
        loadNote(folderName, currentNote.filename, key);
      } else {
        content = "";
        savedContent = "";
        loading = false;
      }
    }
  });

  // Handle hotkey actions
  $effect(() => {
    const unsub = hotkeyAction.subscribe((action) => {
      if (!action) return;
      if (action.type === "toggle-note-preview") {
        noteViewMode.update((mode) => {
          if (mode === "edit") return "preview";
          if (mode === "preview") return "split";
          return "edit";
        });
      }
    });
    return unsub;
  });

  async function loadNote(folder: string, filename: string, requestKey: string) {
    loading = true;
    try {
      const text = await command<string>("read_note", { folder, filename });
      if (prevNoteKey === requestKey) {
        content = text;
        savedContent = text;
      }
    } catch {
      if (prevNoteKey === requestKey) {
        content = "";
        savedContent = "";
      }
    } finally {
      if (prevNoteKey === requestKey) {
        loading = false;
      }
    }
  }

  function scheduleSave() {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      saveNow();
    }, 500);
  }

  async function saveNow() {
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
    if (!currentNote || !folderName || content === savedContent) return;
    try {
      await command("write_note", { folder: folderName, filename: currentNote.filename, content });
      savedContent = content;
    } catch {
      // silently fail — user will see unsaved indicator
    }
  }

  function handleEditorChange(nextContent: string) {
    content = nextContent;
    scheduleSave();
  }

  function handleEditorEscape(mode: VimMode | string) {
    editorMode = mode;
    if (aiChatRequest) {
      aiChatRequest = null;
      return; // consume the escape
    }
    if (editorMode !== "normal") return;

    void saveNow();
    if (currentNote) {
      focusTarget.set({ type: "note", filename: currentNote.filename, folder: currentNote.folder });
    }
  }

</script>

<div class="notes-editor">
  {#if !currentNote}
    <div class="empty-state">
      <div class="empty-title">No note selected</div>
      <div class="empty-hint">press <kbd>n</kbd> to create one</div>
    </div>
  {:else if loading}
    <div class="empty-state">
      <div class="empty-title">Loading...</div>
    </div>
  {:else}
    <div class="editor-header">
      <span class="note-title">{noteTitle}</span>
      {#if isDirty}
        <span class="unsaved-indicator">unsaved</span>
      {/if}
    </div>
    <div class="editor-body" class:focused={editorFocused}>
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
    </div>
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
  {/if}
</div>

<style>
  .notes-editor {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg-void);
    color: var(--text-primary);
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 8px;
  }

  .empty-title {
    font-size: 16px;
    font-weight: 500;
  }

  .empty-hint {
    color: var(--text-secondary);
    font-size: 13px;
  }

  .empty-hint kbd {
    background: var(--bg-hover);
    color: var(--text-emphasis);
    padding: 1px 6px;
    border-radius: 3px;
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .editor-header {
    display: flex;
    align-items: center;
    padding: 10px 16px;
    border-bottom: 1px solid var(--border-default);
    gap: 10px;
    flex-shrink: 0;
  }

  .note-title {
    font-size: 14px;
    font-weight: 600;
    flex: 1;
  }

  .unsaved-indicator {
    font-size: 11px;
    color: var(--status-working);
    font-weight: 500;
  }

  .editor-body {
    flex: 1;
    overflow: hidden;
    border: 2px solid transparent;
    border-radius: 4px;
    transition: border-color 0.15s;
    display: flex;
  }

  .editor-body.focused {
    border-color: var(--focus-ring);
  }

</style>
