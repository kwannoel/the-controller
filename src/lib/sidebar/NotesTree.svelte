<script lang="ts">
  import { command } from "$lib/backend";
  import { fromStore } from "svelte/store";
  import { noteEntries, type NoteEntry, type FocusTarget } from "../stores";

  interface Props {
    folders: string[];
    expandedFolderSet: Set<string>;
    currentFocus: FocusTarget;
    onToggleFolder: (folder: string) => void;
    onFolderFocus: (folder: string) => void;
    onNoteFocus: (filename: string, folder: string) => void;
    onNoteSelect: (filename: string, folder: string) => void;
  }

  let { folders, expandedFolderSet, currentFocus, onToggleFolder, onFolderFocus, onNoteFocus, onNoteSelect }: Props = $props();

  const noteEntriesState = fromStore(noteEntries);
  let noteMap: Map<string, NoteEntry[]> = $derived(noteEntriesState.current);

  function isFolderFocused(folder: string): boolean {
    return currentFocus?.type === "folder" && currentFocus.folder === folder;
  }

  function isNoteFocused(folder: string, filename: string): boolean {
    if (!currentFocus) return false;
    if (currentFocus.type === "note" && currentFocus.folder === folder && currentFocus.filename === filename) return true;
    return false;
  }

  function getNotesForFolder(folder: string): NoteEntry[] {
    return noteMap.get(folder) ?? [];
  }

  function noteCount(folder: string): number {
    return getNotesForFolder(folder).length;
  }

  function displayName(filename: string): string {
    return filename.endsWith(".md") ? filename.slice(0, -3) : filename;
  }

  function fetchNotes(folder: string) {
    command<NoteEntry[]>("list_notes", { folder }).then((entries) => {
      noteEntries.update((map) => {
        const next = new Map(map);
        next.set(folder, entries);
        return next;
      });
    });
  }

  $effect(() => {
    for (const folder of folders) {
      if (expandedFolderSet.has(folder)) {
        fetchNotes(folder);
      }
    }
  });
</script>

{#each folders as folder (folder)}
  <div class="folder-item">
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="folder-header"
      class:focus-target={isFolderFocused(folder)}
      tabindex="0"
      data-folder-id={folder}
      onfocusin={(e: FocusEvent) => {
        if (e.target === e.currentTarget) onFolderFocus(folder);
      }}
    >
      <button class="btn-expand" onclick={() => onToggleFolder(folder)}>
        {expandedFolderSet.has(folder) ? "\u25BC" : "\u25B6"}
      </button>
      <span class="folder-name">{folder}</span>
      <span class="note-count">{noteCount(folder)}</span>
    </div>

    {#if expandedFolderSet.has(folder)}
      <div class="note-list">
        {#each getNotesForFolder(folder) as note (note.filename)}
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
          <div
            class="note-item"
            class:focus-target={isNoteFocused(folder, note.filename)}
            data-note-id="{folder}:{note.filename}"
            tabindex="0"
            onfocusin={() => onNoteFocus(note.filename, folder)}
            ondblclick={() => onNoteSelect(note.filename, folder)}
          >
            <span class="note-name">{displayName(note.filename)}</span>
          </div>
        {/each}

        {#if getNotesForFolder(folder).length === 0}
          <div class="empty-notes">No notes yet — press <kbd>n</kbd></div>
        {/if}
      </div>
    {/if}
  </div>
{/each}

{#if folders.length === 0}
  <div class="empty">No folders — press <kbd>n</kbd> to create a note</div>
{/if}

<style>
  .folder-item {
    border-bottom: 1px solid var(--border-default);
  }

  .folder-header {
    display: flex;
    align-items: center;
    padding: 8px 16px;
    gap: 8px;
  }

  .folder-header:hover {
    background: var(--bg-hover);
  }

  .folder-header.focus-target {
    outline: 2px solid var(--focus-ring);
    outline-offset: -2px;
    border-radius: 4px;
  }

  .btn-expand {
    background: none;
    border: none;
    color: var(--text-secondary);
    cursor: pointer;
    padding: 0;
    font-size: 10px;
    width: 16px;
    text-align: center;
    box-shadow: none;
  }

  .folder-name {
    flex: 1;
    font-size: 13px;
    font-weight: 500;
    word-break: break-word;
  }

  .note-count {
    font-size: 11px;
    color: var(--text-secondary);
    background: var(--bg-hover);
    padding: 1px 6px;
    border-radius: 8px;
  }

  .note-list {
    padding: 0;
  }

  .note-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 16px 6px 40px;
    cursor: pointer;
    font-size: 12px;
    outline: none;
  }

  .note-item:hover {
    background: var(--bg-hover);
  }

  .note-item.focus-target {
    outline: 2px solid var(--focus-ring);
    outline-offset: -2px;
    border-radius: 4px;
  }

  .note-name {
    flex: 1;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .empty-notes {
    padding: 12px 16px 12px 40px;
    color: var(--text-secondary);
    font-size: 12px;
  }

  .empty-notes kbd {
    background: var(--bg-hover);
    padding: 1px 5px;
    border-radius: 3px;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-primary);
  }

  .empty { padding: 16px; color: var(--text-secondary); font-size: 13px; text-align: center; }
</style>
