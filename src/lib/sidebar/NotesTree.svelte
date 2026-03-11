<script lang="ts">
  import { command } from "$lib/backend";
  import { fromStore } from "svelte/store";
  import { noteEntries, type NoteEntry, type Project, type FocusTarget } from "../stores";

  interface Props {
    projects: Project[];
    expandedProjectSet: Set<string>;
    currentFocus: FocusTarget;
    onToggleProject: (projectId: string) => void;
    onProjectFocus: (projectId: string) => void;
    onNoteFocus: (filename: string, projectId: string) => void;
    onNoteSelect: (filename: string, projectId: string) => void;
  }

  let { projects, expandedProjectSet, currentFocus, onToggleProject, onProjectFocus, onNoteFocus, onNoteSelect }: Props = $props();

  const noteEntriesState = fromStore(noteEntries);
  let noteMap: Map<string, NoteEntry[]> = $derived(noteEntriesState.current);

  function isProjectFocused(projectId: string): boolean {
    return currentFocus?.type === "project" && currentFocus.projectId === projectId;
  }

  function isNoteFocused(projectId: string, filename: string): boolean {
    if (!currentFocus) return false;
    if (currentFocus.type === "note" && currentFocus.projectId === projectId && currentFocus.filename === filename) return true;
    return false;
  }

  function getNotesForProject(projectId: string): NoteEntry[] {
    return noteMap.get(projectId) ?? [];
  }

  function noteCount(projectId: string): number {
    return getNotesForProject(projectId).length;
  }

  function displayName(filename: string): string {
    return filename.endsWith(".md") ? filename.slice(0, -3) : filename;
  }

  function fetchNotes(project: Project) {
    command<NoteEntry[]>("list_notes", { projectName: project.name }).then((entries) => {
      noteEntries.update((map) => {
        const next = new Map(map);
        next.set(project.id, entries);
        return next;
      });
    });
  }

  $effect(() => {
    for (const project of projects) {
      if (expandedProjectSet.has(project.id)) {
        fetchNotes(project);
      }
    }
  });
</script>

{#each projects as project (project.id)}
  <div class="project-item">
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="project-header"
      class:focus-target={isProjectFocused(project.id)}
      tabindex="0"
      data-project-id={project.id}
      onfocusin={(e: FocusEvent) => {
        if (e.target === e.currentTarget) onProjectFocus(project.id);
      }}
    >
      <button class="btn-expand" onclick={() => onToggleProject(project.id)}>
        {expandedProjectSet.has(project.id) ? "\u25BC" : "\u25B6"}
      </button>
      <span class="project-name">{project.name}</span>
      <span class="note-count">{noteCount(project.id)}</span>
    </div>

    {#if expandedProjectSet.has(project.id)}
      <div class="note-list">
        {#each getNotesForProject(project.id) as note (note.filename)}
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
          <div
            class="note-item"
            class:focus-target={isNoteFocused(project.id, note.filename)}
            data-note-id="{project.id}:{note.filename}"
            tabindex="0"
            onfocusin={() => onNoteFocus(note.filename, project.id)}
            ondblclick={() => onNoteSelect(note.filename, project.id)}
          >
            <span class="note-name">{displayName(note.filename)}</span>
          </div>
        {/each}

        {#if getNotesForProject(project.id).length === 0}
          <div class="empty-notes">No notes yet — press <kbd>n</kbd></div>
        {/if}
      </div>
    {/if}
  </div>
{/each}

{#if projects.length === 0}
  <div class="empty">No projects</div>
{/if}

<style>
  .project-item {
    border-bottom: 1px solid var(--border-default);
  }

  .project-header {
    display: flex;
    align-items: center;
    padding: 8px 16px;
    gap: 8px;
  }

  .project-header:hover {
    background: var(--bg-hover);
  }

  .project-header.focus-target {
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

  .project-name {
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
