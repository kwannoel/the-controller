<script lang="ts">
  import { onMount } from "svelte";

  interface Props {
    onSubmit: (title: string) => void;
    onClose: () => void;
  }

  let { onSubmit, onClose }: Props = $props();

  let title = $state("");
  let titleInput: HTMLInputElement | undefined = $state();

  onMount(() => {
    titleInput?.focus();
  });

  function submit() {
    if (!title.trim()) return;
    onSubmit(title.trim());
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
</script>

<div class="overlay" onclick={onClose} onkeydown={handleKeydown} role="dialog">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal" onclick={(e) => e.stopPropagation()} role="presentation">
    <div class="modal-header">New Note</div>
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
        disabled={!title.trim()}
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
