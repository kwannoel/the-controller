<script lang="ts">
  import { onMount, tick } from "svelte";

  const NEW_FOLDER_SENTINEL = "__new_folder__";

  interface Props {
    folders: string[];
    onSubmit: (title: string, folder: string) => void;
    onClose: () => void;
  }

  let { folders, onSubmit, onClose }: Props = $props();

  let title = $state("");
  let selectedFolder = $state(NEW_FOLDER_SENTINEL);
  let newFolderName = $state("");
  let folderSelectEl: HTMLSelectElement | undefined = $state();
  let newFolderInput: HTMLInputElement | undefined = $state();
  let titleInput: HTMLInputElement | undefined = $state();

  let isNewFolder = $derived(selectedFolder === NEW_FOLDER_SENTINEL);
  let resolvedFolder = $derived(isNewFolder ? newFolderName.trim() : selectedFolder);
  let canSubmit = $derived(title.trim() !== "" && resolvedFolder !== "");

  $effect(() => {
    if (folders.length > 0 && selectedFolder === NEW_FOLDER_SENTINEL) {
      selectedFolder = folders[0];
      tick().then(() => titleInput?.focus());
    }
  });

  onMount(async () => {
    await tick();
    if (isNewFolder) {
      newFolderInput?.focus();
    } else {
      titleInput?.focus();
    }
  });

  function submit() {
    if (!canSubmit) return;
    onSubmit(title.trim(), resolvedFolder);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    } else if (e.key === "Enter") {
      e.preventDefault();
      submit();
    }
  }

  function handleFolderChange() {
    if (isNewFolder) {
      // Wait a tick for the input to render, then focus it
      requestAnimationFrame(() => newFolderInput?.focus());
    }
  }
</script>

<div
  class="overlay"
  onclick={onClose}
  onkeydown={handleKeydown}
  role="dialog"
  tabindex="-1"
  aria-modal="true"
>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal" onclick={(e) => e.stopPropagation()} role="presentation">
    <div class="modal-header">New Note</div>
    <select
      bind:this={folderSelectEl}
      bind:value={selectedFolder}
      class="input"
      onchange={handleFolderChange}
    >
      {#each folders as f}
        <option value={f}>{f}</option>
      {/each}
      <option value={NEW_FOLDER_SENTINEL}>New folder...</option>
    </select>
    {#if isNewFolder}
      <input
        bind:this={newFolderInput}
        bind:value={newFolderName}
        placeholder="Folder name"
        class="input"
      />
    {/if}
    <input
      bind:this={titleInput}
      bind:value={title}
      placeholder="Note title"
      class="input"
    />
    <div class="actions">
      <button class="btn-cancel" onclick={onClose}>Cancel</button>
      <button
        class="btn-primary"
        onclick={submit}
        disabled={!canSubmit}
      >
        Create
      </button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(16px);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 20vh;
    z-index: 100;
  }
  .modal {
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: 8px;
    width: 380px;
    padding: 20px 24px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
  }
  .modal-header {
    font-size: 14px;
    font-weight: 600;
    color: var(--text-emphasis);
  }
  .input {
    background: var(--bg-void);
    color: var(--text-primary);
    border: 1px solid var(--border-default);
    padding: 10px 12px;
    border-radius: 6px;
    font-size: 14px;
    outline: none;
    width: 100%;
    box-sizing: border-box;
  }
  .input:focus {
    border-color: var(--text-emphasis);
  }
  select.input {
    appearance: none;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 12 12'%3E%3Cpath fill='%23cdd6f4' d='M2 4l4 4 4-4'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 12px center;
    padding-right: 32px;
    cursor: pointer;
  }
  select.input option {
    background: var(--bg-void);
    color: var(--text-primary);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .btn-cancel {
    background: var(--bg-hover);
    color: var(--text-primary);
    border: none;
    padding: 10px 16px;
    border-radius: 6px;
    font-size: 13px;
    cursor: pointer;
  }
  .btn-cancel:hover {
    background: var(--bg-active);
  }
  .btn-primary {
    background: var(--text-emphasis);
    color: var(--bg-void);
    border: none;
    padding: 10px 16px;
    border-radius: 6px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
  }
  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
