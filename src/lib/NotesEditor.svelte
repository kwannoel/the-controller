<script lang="ts">
  import { fromStore } from "svelte/store";
  import { untrack } from "svelte";
  import { command } from "$lib/backend";
  import { activeNote, noteViewMode, projects, focusTarget, hotkeyAction, type NoteViewMode, type Project, type FocusTarget } from "./stores";
  import CodeMirrorNoteEditor, { type VimMode, type AiChatRequest } from "./CodeMirrorNoteEditor.svelte";
  import NoteAiPanel from "./NoteAiPanel.svelte";
  import { renderMarkdown } from "./markdown";

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

  const projectsState = fromStore(projects);
  let projectList: Project[] = $derived(projectsState.current);

  const focusTargetState = fromStore(focusTarget);
  let currentFocus: FocusTarget = $derived(focusTargetState.current);

  let editorFocused = $derived(currentFocus?.type === "notes-editor");
  let editorEntryKey = $derived(currentFocus?.type === "notes-editor" ? currentFocus.entryKey : undefined);

  let projectName = $derived(
    currentNote
      ? projectList.find((p) => p.id === currentNote!.projectId)?.name ?? null
      : null
  );

  let noteTitle = $derived(
    currentNote
      ? currentNote.filename.replace(/\.md$/, "")
      : null
  );

  let isDirty = $derived(content !== savedContent);

  let renderedHtml = $derived(renderMarkdown(content));

  // Load note content when activeNote changes
  let prevNoteKey: string | null = $state(null);
  $effect(() => {
    const key = currentNote ? `${currentNote.projectId}:${currentNote.filename}` : null;
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
          const [prevProjectId, ...rest] = prev.split(":");
          const prevFilename = rest.join(":");
          const prevProjectName = untrack(() => projectList.find(p => p.id === prevProjectId)?.name);
          if (prevProjectName && prevFilename) {
            command("write_note", { projectName: prevProjectName, filename: prevFilename, content: prevContent }).catch(() => {});
          }
        }
      }
      prevNoteKey = key;

      if (currentNote && projectName && key) {
        loadNote(projectName, currentNote.filename, key);
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

  async function loadNote(pName: string, filename: string, requestKey: string) {
    loading = true;
    try {
      const text = await command<string>("read_note", { projectName: pName, filename });
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
    if (!currentNote || !projectName || content === savedContent) return;
    try {
      await command("write_note", { projectName, filename: currentNote.filename, content });
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
      focusTarget.set({ type: "note", filename: currentNote.filename, projectId: currentNote.projectId });
    }
  }

  function setViewMode(mode: NoteViewMode) {
    noteViewMode.set(mode);
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
      <div class="view-mode-controls">
        <button
          class="view-mode-button"
          class:active={currentViewMode === "edit"}
          onclick={() => setViewMode("edit")}
        >
          Edit
        </button>
        <button
          class="view-mode-button"
          class:active={currentViewMode === "preview"}
          onclick={() => setViewMode("preview")}
        >
          Preview
        </button>
        <button
          class="view-mode-button"
          class:active={currentViewMode === "split"}
          onclick={() => setViewMode("split")}
        >
          Split
        </button>
      </div>
    </div>
    <div class="editor-body" class:focused={editorFocused} class:split={currentViewMode === "split"}>
      {#if showsEditor}
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
      {/if}
      {#if showsPreview}
        <div class="preview" class:split={currentViewMode === "split"}>
          {@html renderedHtml}
        </div>
      {/if}
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
    background: #11111b;
    color: #cdd6f4;
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
    color: #6c7086;
    font-size: 13px;
  }

  .empty-hint kbd {
    background: #313244;
    color: #89b4fa;
    padding: 1px 6px;
    border-radius: 3px;
    font-family: monospace;
    font-size: 12px;
  }

  .editor-header {
    display: flex;
    align-items: center;
    padding: 10px 16px;
    border-bottom: 1px solid #313244;
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
    color: #f9e2af;
    font-weight: 500;
  }

  .view-mode-controls {
    display: flex;
    gap: 6px;
  }

  .view-mode-button {
    background: #313244;
    border: none;
    color: #cdd6f4;
    padding: 4px 10px;
    border-radius: 4px;
    font-size: 11px;
    cursor: pointer;
  }

  .view-mode-button:hover {
    background: #45475a;
  }

  .view-mode-button.active {
    background: #89b4fa;
    color: #1e1e2e;
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
    border-color: #89b4fa;
  }

  .editor-body.split {
    gap: 1px;
    background: #313244;
  }

  .preview {
    padding: 16px;
    overflow-y: auto;
    height: 100%;
    box-sizing: border-box;
    flex: 1;
    background: #11111b;
  }

  .preview.split {
    border-left: 1px solid #313244;
  }

  .preview :global(h1) {
    font-size: 24px;
    font-weight: 700;
    margin: 0 0 12px;
    border-bottom: 1px solid #313244;
    padding-bottom: 8px;
  }

  .preview :global(h2) {
    font-size: 20px;
    font-weight: 600;
    margin: 16px 0 8px;
  }

  .preview :global(h3) {
    font-size: 16px;
    font-weight: 600;
    margin: 12px 0 6px;
  }

  .preview :global(h4),
  .preview :global(h5),
  .preview :global(h6) {
    font-size: 14px;
    font-weight: 600;
    margin: 10px 0 4px;
  }

  .preview :global(p) {
    margin: 0 0 8px;
    line-height: 1.6;
  }

  .preview :global(br) {
    display: block;
    margin: 4px 0;
  }

  .preview :global(strong) {
    font-weight: 700;
  }

  .preview :global(em) {
    font-style: italic;
  }

  .preview :global(a) {
    color: #89b4fa;
    text-decoration: none;
  }

  .preview :global(a:hover) {
    text-decoration: underline;
  }

  .preview :global(code) {
    background: #1e1e2e;
    padding: 2px 5px;
    border-radius: 3px;
    font-family: monospace;
    font-size: 13px;
  }

  .preview :global(pre) {
    background: #1e1e2e;
    padding: 12px 16px;
    border-radius: 6px;
    overflow-x: auto;
    margin: 8px 0;
  }

  .preview :global(pre code) {
    background: none;
    padding: 0;
    font-size: 13px;
    line-height: 1.5;
  }

  .preview :global(ul) {
    margin: 4px 0 8px;
    padding-left: 20px;
  }

  .preview :global(li) {
    margin: 2px 0;
    line-height: 1.6;
  }
</style>
