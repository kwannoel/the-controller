<script lang="ts">
  import { fromStore } from "svelte/store";
  import { untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { activeNote, notePreviewMode, projects, focusTarget, hotkeyAction, type Project, type FocusTarget } from "./stores";
  import { renderMarkdown } from "./markdown";

  let content = $state("");
  let savedContent = $state("");
  let loading = $state(true);
  let saveTimer: ReturnType<typeof setTimeout> | null = $state(null);
  let textareaEl: HTMLTextAreaElement | undefined = $state(undefined);

  const activeNoteState = fromStore(activeNote);
  let currentNote = $derived(activeNoteState.current);

  const previewModeState = fromStore(notePreviewMode);
  let isPreview = $derived(previewModeState.current);

  const projectsState = fromStore(projects);
  let projectList: Project[] = $derived(projectsState.current);

  const focusTargetState = fromStore(focusTarget);
  let currentFocus: FocusTarget = $derived(focusTargetState.current);

  let editorFocused = $derived(currentFocus?.type === "notes-editor");

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
            invoke("write_note", { projectName: prevProjectName, filename: prevFilename, content: prevContent }).catch(() => {});
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
        notePreviewMode.update((v) => !v);
      }
    });
    return unsub;
  });

  // Auto-focus textarea when focusTarget is notes-editor
  $effect(() => {
    if (currentFocus?.type === "notes-editor" && textareaEl && !isPreview) {
      textareaEl.focus();
    }
  });

  async function loadNote(pName: string, filename: string, requestKey: string) {
    loading = true;
    try {
      const text = await invoke<string>("read_note", { projectName: pName, filename });
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
      await invoke("write_note", { projectName, filename: currentNote.filename, content });
      savedContent = content;
    } catch {
      // silently fail — user will see unsaved indicator
    }
  }

  function handleInput() {
    scheduleSave();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      saveNow();
      // Return focus to sidebar note item
      if (currentNote) {
        focusTarget.set({ type: "note", filename: currentNote.filename, projectId: currentNote.projectId });
      }
    } else if (e.key === "Tab") {
      e.preventDefault();
      if (textareaEl) {
        const start = textareaEl.selectionStart;
        const end = textareaEl.selectionEnd;
        content = content.substring(0, start) + "  " + content.substring(end);
        // Restore cursor after the inserted spaces
        requestAnimationFrame(() => {
          if (textareaEl) {
            textareaEl.selectionStart = start + 2;
            textareaEl.selectionEnd = start + 2;
          }
        });
        scheduleSave();
      }
    }
  }

  function togglePreview() {
    notePreviewMode.update((v) => !v);
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
      <button
        class="preview-toggle"
        class:active={isPreview}
        onclick={togglePreview}
      >
        {isPreview ? "Edit" : "Preview"}
      </button>
    </div>
    <div class="editor-body" class:focused={editorFocused}>
      {#if isPreview}
        <div class="preview">
          {@html renderedHtml}
        </div>
      {:else}
        <textarea
          bind:this={textareaEl}
          bind:value={content}
          oninput={handleInput}
          onkeydown={handleKeydown}
          class="editor-textarea"
          spellcheck="false"
          placeholder="Start writing..."
        ></textarea>
      {/if}
    </div>
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

  .preview-toggle {
    background: #313244;
    border: none;
    color: #cdd6f4;
    padding: 4px 10px;
    border-radius: 4px;
    font-size: 11px;
    cursor: pointer;
  }

  .preview-toggle:hover {
    background: #45475a;
  }

  .preview-toggle.active {
    background: #89b4fa;
    color: #1e1e2e;
  }

  .editor-body {
    flex: 1;
    overflow: hidden;
    border: 2px solid transparent;
    border-radius: 4px;
    transition: border-color 0.15s;
  }

  .editor-body.focused {
    border-color: #89b4fa;
  }

  .editor-textarea {
    width: 100%;
    height: 100%;
    background: transparent;
    color: #cdd6f4;
    border: none;
    outline: none;
    resize: none;
    padding: 16px;
    font-family: monospace;
    font-size: 14px;
    line-height: 1.6;
    box-sizing: border-box;
  }

  .editor-textarea::placeholder {
    color: #6c7086;
  }

  .preview {
    padding: 16px;
    overflow-y: auto;
    height: 100%;
    box-sizing: border-box;
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
