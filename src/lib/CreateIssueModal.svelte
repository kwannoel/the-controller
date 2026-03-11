<script lang="ts">
  import { onMount } from "svelte";

  type Priority = "high" | "low";
  type Complexity = "high" | "low";

  interface Props {
    onSubmit: (title: string, priority: Priority, complexity: Complexity) => void;
    onClose: () => void;
  }

  let { onSubmit, onClose }: Props = $props();

  let title = $state("");
  let stage: "title" | "priority" | "complexity" = $state("title");
  let selectedPriority: Priority = $state("low");
  let titleInput: HTMLInputElement | undefined = $state();
  let overlayEl: HTMLDivElement | undefined = $state();

  onMount(() => {
    titleInput?.focus();
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      if (stage === "complexity") {
        stage = "priority";
        requestAnimationFrame(() => overlayEl?.focus());
      } else if (stage === "priority") {
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
        selectedPriority = "low";
        stage = "complexity";
        requestAnimationFrame(() => overlayEl?.focus());
      } else if (e.key === "k") {
        e.preventDefault();
        selectedPriority = "high";
        stage = "complexity";
        requestAnimationFrame(() => overlayEl?.focus());
      }
      return;
    }

    if (stage === "complexity") {
      if (e.key === "j") {
        e.preventDefault();
        onSubmit(title.trim(), selectedPriority, "low");
      } else if (e.key === "k") {
        e.preventDefault();
        onSubmit(title.trim(), selectedPriority, "high");
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
    {:else if stage === "priority"}
      <div class="title-preview">{title}</div>
      <div class="priority-prompt">
        <span class="priority-key low">j</span> Low Priority
        <span class="priority-key high">k</span> High Priority
      </div>
      <div class="hint">Press Esc to go back</div>
    {:else}
      <div class="title-preview">{title}</div>
      <div class="selected-badge {selectedPriority}">{selectedPriority} priority</div>
      <div class="priority-prompt">
        <span class="priority-key simple">j</span> Low Complexity
        <span class="priority-key complex">k</span> High Complexity
      </div>
      <div class="hint">Press Esc to go back</div>
    {/if}
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
    outline: none;
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
  .hint {
    color: var(--text-secondary);
    font-size: 12px;
    text-align: center;
  }
  .title-preview {
    color: var(--text-primary);
    font-size: 14px;
    padding: 10px 12px;
    background: var(--bg-hover);
    border-radius: 6px;
  }
  .priority-prompt {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 20px;
    font-size: 15px;
    color: var(--text-primary);
    padding: 8px 0;
  }
  .priority-key {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    background: var(--bg-hover);
    border: 1px solid var(--border-default);
    border-radius: 4px;
    font-size: 13px;
    font-weight: 600;
    margin-right: 4px;
  }
  .priority-key.high {
    color: var(--status-error);
    border-color: var(--status-error);
  }
  .priority-key.low {
    color: var(--status-idle);
    border-color: var(--status-idle);
  }
  .priority-key.simple {
    color: var(--text-emphasis);
    border-color: var(--text-emphasis);
  }
  .priority-key.complex {
    color: var(--status-working);
    border-color: var(--status-working);
  }
  .selected-badge {
    font-size: 12px;
    padding: 4px 10px;
    border-radius: 4px;
    text-align: center;
    text-transform: capitalize;
  }
  .selected-badge.high {
    color: var(--status-error);
    background: rgba(196, 64, 64, 0.1);
  }
  .selected-badge.low {
    color: var(--status-idle);
    background: rgba(74, 158, 110, 0.1);
  }
</style>
