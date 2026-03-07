<script lang="ts">
  import { onMount } from "svelte";

  type Priority = "high" | "low";

  interface Props {
    onSubmit: (title: string, priority: Priority) => void;
    onClose: () => void;
  }

  let { onSubmit, onClose }: Props = $props();

  let title = $state("");
  let priority: Priority = $state("low");
  let titleInput: HTMLInputElement | undefined = $state();

  onMount(() => {
    titleInput?.focus();
  });

  function submit() {
    if (!title.trim()) return;
    onSubmit(title.trim(), priority);
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
    <div class="modal-header">New GitHub Issue</div>
    <input
      bind:this={titleInput}
      bind:value={title}
      placeholder="Issue title"
      class="input"
    />
    <div class="priority-row">
      <span class="priority-label">Priority:</span>
      <div class="priority-buttons">
        <button
          class="priority-btn high"
          class:selected={priority === "high"}
          onclick={() => priority = "high"}
          type="button"
        >High</button>
        <button
          class="priority-btn low"
          class:selected={priority === "low"}
          onclick={() => priority = "low"}
          type="button"
        >Low</button>
      </div>
    </div>
    <button
      class="btn-primary"
      onclick={submit}
      disabled={!title.trim()}
    >
      Create Issue
    </button>
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
  }
  .modal-header {
    font-size: 16px;
    font-weight: 600;
    color: #cdd6f4;
  }
  .input {
    background: #313244;
    color: #cdd6f4;
    border: 1px solid #45475a;
    padding: 10px 12px;
    border-radius: 6px;
    font-size: 14px;
    outline: none;
    width: 100%;
    box-sizing: border-box;
  }
  .input:focus {
    border-color: #89b4fa;
  }
  .btn-primary {
    background: #89b4fa;
    color: #1e1e2e;
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
  .priority-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .priority-label {
    color: #a6adc8;
    font-size: 13px;
    flex-shrink: 0;
  }
  .priority-buttons {
    display: flex;
    gap: 6px;
  }
  .priority-btn {
    background: #313244;
    color: #a6adc8;
    border: 1px solid #45475a;
    padding: 4px 12px;
    border-radius: 4px;
    font-size: 13px;
    cursor: pointer;
  }
  .priority-btn.high.selected {
    border-color: #f38ba8;
    color: #f38ba8;
  }
  .priority-btn.low.selected {
    border-color: #a6e3a1;
    color: #a6e3a1;
  }
</style>
