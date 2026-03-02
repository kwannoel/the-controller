<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
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

  onMount(() => {
    modalEl?.focus();
  });

  async function deleteProject(deleteRepo: boolean) {
    if (loading) return;
    loading = true;
    try {
      await invoke("delete_project", { projectId, deleteRepo });
      onDeleted();
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      loading = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    }
  }
</script>

<div class="overlay" onclick={onClose} onkeydown={handleKeydown} role="dialog">
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
      >Untrack</button>
      <button
        class="btn-delete"
        onclick={() => deleteProject(true)}
        disabled={loading}
      >Delete Everything</button>
      <button
        class="btn-cancel"
        onclick={onClose}
        disabled={loading}
      >Cancel</button>
    </div>
    <p class="hint">Untrack removes from the controller only. Delete Everything also removes the repo directory.</p>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 20vh;
    z-index: 100;
  }
  .modal {
    background: #1e1e2e;
    border: 1px solid #313244;
    border-radius: 8px;
    width: 420px;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    outline: none;
  }
  .modal-header {
    font-size: 16px;
    font-weight: 600;
    color: #cdd6f4;
  }
  .description {
    color: #a6adc8;
    font-size: 13px;
    margin: 0;
    line-height: 1.5;
  }
  .description strong {
    color: #cdd6f4;
  }
  .hint {
    color: #6c7086;
    font-size: 11px;
    margin: 0;
    line-height: 1.4;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  .btn-untrack {
    background: #45475a;
    color: #cdd6f4;
    border: none;
    padding: 10px 16px;
    border-radius: 6px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
  }
  .btn-untrack:hover {
    background: #585b70;
  }
  .btn-delete {
    background: #f38ba8;
    color: #1e1e2e;
    border: none;
    padding: 10px 16px;
    border-radius: 6px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
  }
  .btn-delete:hover {
    background: #eba0ac;
  }
  .btn-cancel {
    background: none;
    color: #6c7086;
    border: 1px solid #313244;
    padding: 10px 16px;
    border-radius: 6px;
    font-size: 13px;
    cursor: pointer;
    margin-left: auto;
  }
  .btn-cancel:hover {
    color: #cdd6f4;
    border-color: #45475a;
  }
  .btn-untrack:disabled,
  .btn-delete:disabled,
  .btn-cancel:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
