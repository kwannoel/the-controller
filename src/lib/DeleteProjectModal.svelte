<script lang="ts">
  import { command } from "$lib/backend";
  import { onMount } from "svelte";
  import { showToast } from "./toast";

  interface Props {
    projectId: string;
    projectName: string;
    onDeleted: () => void;
    onClose: () => void;
  }

  let { projectId, projectName, onDeleted, onClose }: Props = $props();

  let loading = $state(false);
  let modalEl: HTMLDivElement | undefined = $state();

  async function deleteProject(deleteRepo: boolean) {
    if (loading) return;
    loading = true;
    try {
      await command("delete_project", { projectId, deleteRepo });
      onDeleted();
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      loading = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" || e.key === "n") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    } else if (e.key === "u") {
      e.preventDefault();
      e.stopPropagation();
      deleteProject(false);
    } else if (e.key === "d") {
      e.preventDefault();
      e.stopPropagation();
      deleteProject(true);
    }
  }

  onMount(() => {
    modalEl?.focus();
    window.addEventListener("keydown", handleKeydown, { capture: true });
    return () => {
      window.removeEventListener("keydown", handleKeydown, { capture: true });
    };
  });
</script>

<div class="overlay" onclick={onClose} role="dialog">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="modal"
    bind:this={modalEl}
    onclick={(e) => e.stopPropagation()}
    role="presentation"
    tabindex="-1"
  >
    <div class="modal-header">Delete Project</div>
    <p class="description">
      Delete <strong>{projectName}</strong>? This will close all sessions and remove worktrees.
    </p>
    <div class="actions">
      <button
        class="btn-untrack"
        onclick={() => deleteProject(false)}
        disabled={loading}
      >Untrack <kbd>u</kbd></button>
      <button
        class="btn-delete"
        onclick={() => deleteProject(true)}
        disabled={loading}
      >Delete Everything <kbd>d</kbd></button>
      <button
        class="btn-cancel"
        onclick={onClose}
        disabled={loading}
      >Cancel <kbd>n</kbd></button>
    </div>
    <p class="hint">Untrack removes from the controller only. Delete Everything also removes the repo directory.</p>
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
    width: 420px;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    outline: none;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
  }
  .modal-header {
    font-size: 16px;
    font-weight: 600;
    color: var(--text-emphasis);
  }
  .description {
    color: var(--text-secondary);
    font-size: 13px;
    margin: 0;
    line-height: 1.5;
  }
  .description strong {
    color: var(--text-primary);
  }
  .hint {
    color: var(--text-secondary);
    font-size: 11px;
    margin: 0;
    line-height: 1.4;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  .btn-untrack {
    background: var(--bg-active);
    color: var(--text-primary);
    border: none;
    padding: 10px 16px;
    border-radius: 6px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
  }
  .btn-untrack:hover {
    opacity: 0.85;
  }
  .btn-delete {
    background: var(--status-error);
    color: var(--text-emphasis);
    border: none;
    padding: 10px 16px;
    border-radius: 6px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
  }
  .btn-delete:hover {
    opacity: 0.85;
  }
  .btn-cancel {
    background: none;
    color: var(--text-secondary);
    border: 1px solid var(--border-default);
    padding: 10px 16px;
    border-radius: 6px;
    font-size: 13px;
    cursor: pointer;
    margin-left: auto;
  }
  .btn-cancel:hover {
    color: var(--text-primary);
    border-color: var(--text-secondary);
  }
  .btn-untrack:disabled,
  .btn-delete:disabled,
  .btn-cancel:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  kbd {
    font-family: var(--font-mono);
    font-size: 11px;
    opacity: 0.7;
  }
</style>
