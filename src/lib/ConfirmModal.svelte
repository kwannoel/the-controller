<script lang="ts">
  import { onMount } from "svelte";

  interface Props {
    title: string;
    message: string;
    confirmLabel?: string;
    onConfirm: () => void;
    onClose: () => void;
  }

  let { title, message, confirmLabel = "Confirm", onConfirm, onClose }: Props = $props();
  let modalEl: HTMLDivElement | undefined = $state();

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" || e.key === "n") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    } else if (e.key === "y") {
      e.preventDefault();
      e.stopPropagation();
      onConfirm();
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

<div
  class="overlay"
  onclick={onClose}
  onkeydown={handleKeydown}
  role="dialog"
  tabindex="-1"
  aria-modal="true"
>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="modal"
    bind:this={modalEl}
    onclick={(e) => e.stopPropagation()}
    role="presentation"
    tabindex="-1"
  >
    <div class="modal-header">{title}</div>
    <p class="description">{message}</p>
    <div class="actions">
      <button class="btn-confirm" onclick={onConfirm}>{confirmLabel} <kbd>y</kbd></button>
      <button class="btn-cancel" onclick={onClose}>Cancel <kbd>n</kbd></button>
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
  .actions {
    display: flex;
    gap: 8px;
  }
  .btn-confirm {
    background: var(--status-error);
    color: var(--text-emphasis);
    border: none;
    padding: 10px 16px;
    border-radius: 6px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
  }
  .btn-confirm:hover {
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
  kbd {
    font-family: var(--font-mono);
    font-size: 11px;
    opacity: 0.7;
  }
</style>
