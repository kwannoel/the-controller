<script lang="ts">
  import { command } from "$lib/backend";
  import { onMount } from "svelte";
  import { showToast } from "./toast";
  import type { Project } from "./stores";

  interface Props {
    onCreated: (project: Project) => void;
    onClose: () => void;
  }

  let { onCreated, onClose }: Props = $props();

  let name = $state("");
  let loading = $state(false);
  let nameInput: HTMLInputElement | undefined = $state();

  onMount(() => {
    nameInput?.focus();
  });

  async function create() {
    if (!name.trim() || loading) return;
    loading = true;
    try {
      const project = await command<Project>("scaffold_project", {
        name: name.trim(),
      });
      onCreated(project);
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
    } else if (e.key === "Enter") {
      e.preventDefault();
      create();
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
    <div class="modal-header">New Project</div>
    <input
      bind:this={nameInput}
      bind:value={name}
      placeholder="Project name"
      class="input"
      disabled={loading}
    />
    <button
      class="btn-primary"
      onclick={create}
      disabled={!name.trim() || loading}
    >
      {loading ? "Creating..." : "Create"}
    </button>
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
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
  }
  .modal-header {
    font-size: 16px;
    font-weight: 600;
    color: var(--text-emphasis);
  }
  .input {
    background: var(--bg-hover);
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
  .btn-primary {
    background: var(--text-emphasis);
    color: var(--bg-void);
    border: none;
    padding: 10px;
    border-radius: 6px;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
  }
  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
