<script lang="ts">
  import { onMount } from "svelte";

  type Priority = "high" | "low";

  interface Props {
    onSubmit: (title: string, priority: Priority) => void;
    onClose: () => void;
  }

  let { onSubmit, onClose }: Props = $props();

  let title = $state("");
  let stage: "title" | "priority" = $state("title");
  let titleInput: HTMLInputElement | undefined = $state();
  let overlayEl: HTMLDivElement | undefined = $state();

  onMount(() => {
    titleInput?.focus();
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      if (stage === "priority") {
        stage = "title";
        requestAnimationFrame(() => titleInput?.focus());
      } else {
        onClose();
      }
      return;
    }

    if (stage === "title" && e.key === "Enter") {
      e.preventDefault();
      if (!title.trim()) return;
      stage = "priority";
      requestAnimationFrame(() => overlayEl?.focus());
      return;
    }

    if (stage === "priority") {
      if (e.key === "j") {
        e.preventDefault();
        onSubmit(title.trim(), "low");
      } else if (e.key === "k") {
        e.preventDefault();
        onSubmit(title.trim(), "high");
      }
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div class="overlay" bind:this={overlayEl} tabindex="0" onclick={onClose} onkeydown={handleKeydown} role="dialog">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal" onclick={(e) => e.stopPropagation()} role="presentation">
    <div class="modal-header">New GitHub Issue</div>
    {#if stage === "title"}
      <input
        bind:this={titleInput}
        bind:value={title}
        placeholder="Issue title"
        class="input"
      />
      <div class="hint">Press Enter to continue</div>
    {:else}
      <div class="title-preview">{title}</div>
      <div class="priority-prompt">
        <span class="priority-key low">j</span> Low Priority
        <span class="priority-key high">k</span> High Priority
      </div>
      <div class="hint">Press Esc to go back</div>
    {/if}
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
    outline: none;
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
  .hint {
    color: #585b70;
    font-size: 12px;
    text-align: center;
  }
  .title-preview {
    color: #cdd6f4;
    font-size: 14px;
    padding: 10px 12px;
    background: #313244;
    border-radius: 6px;
  }
  .priority-prompt {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 20px;
    font-size: 15px;
    color: #cdd6f4;
    padding: 8px 0;
  }
  .priority-key {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    background: #313244;
    border: 1px solid #45475a;
    border-radius: 4px;
    font-size: 13px;
    font-weight: 600;
    margin-right: 4px;
  }
  .priority-key.high {
    color: #f38ba8;
    border-color: #f38ba8;
  }
  .priority-key.low {
    color: #a6e3a1;
    border-color: #a6e3a1;
  }
</style>
