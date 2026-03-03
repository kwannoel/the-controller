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

  onMount(() => {
    modalEl?.focus();
  });

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
    width: 380px;
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
  .actions {
    display: flex;
    gap: 8px;
  }
  .btn-confirm {
    background: #f38ba8;
    color: #1e1e2e;
    border: none;
    padding: 10px 16px;
    border-radius: 6px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
  }
  .btn-confirm:hover {
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
  kbd {
    font-family: monospace;
    font-size: 11px;
    opacity: 0.7;
  }
</style>
