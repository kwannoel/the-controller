<script lang="ts">
  import { onMount } from "svelte";
  import { command } from "$lib/backend";
  import type { SavedPrompt } from "./stores";

  interface Props {
    projectId: string;
    onSelect: (prompt: SavedPrompt) => void;
    onClose: () => void;
  }

  let { projectId, onSelect, onClose }: Props = $props();

  let prompts: SavedPrompt[] = $state([]);
  let loading = $state(true);
  let error: string | null = $state(null);
  let selectedIndex = $state(0);

  onMount(() => {
    window.addEventListener("keydown", handleKeydown, { capture: true });

    (async () => {
      try {
        prompts = await command<SavedPrompt[]>("list_project_prompts", { projectId });
      } catch (e) {
        error = String(e);
      } finally {
        loading = false;
      }
    })();

    return () => {
      window.removeEventListener("keydown", handleKeydown, { capture: true });
    };
  });

  function confirm() {
    if (prompts.length > 0) {
      onSelect(prompts[selectedIndex]);
    }
  }

  function scrollSelectedIntoView() {
    requestAnimationFrame(() => {
      const el = document.querySelector('.prompt-list .prompt-btn.selected');
      el?.scrollIntoView({ block: 'nearest' });
    });
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
      return;
    }

    if (loading || error || prompts.length === 0) return;

    switch (e.key) {
      case "j":
        e.preventDefault();
        e.stopPropagation();
        selectedIndex = (selectedIndex + 1) % prompts.length;
        scrollSelectedIntoView();
        break;
      case "k":
        e.preventDefault();
        e.stopPropagation();
        selectedIndex = (selectedIndex - 1 + prompts.length) % prompts.length;
        scrollSelectedIntoView();
        break;
      case "l":
      case "Enter":
        e.preventDefault();
        e.stopPropagation();
        confirm();
        break;
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
    <div class="modal-header">Load Saved Prompt</div>
    {#if loading}
      <div class="status">Loading prompts...</div>
    {:else if error}
      <div class="status error">{error}</div>
    {:else if prompts.length === 0}
      <div class="status">No saved prompts</div>
    {:else}
      <ul class="prompt-list">
        {#each prompts as prompt, index (prompt.id)}
          <li>
            <button class="prompt-btn" class:selected={selectedIndex === index} onclick={() => onSelect(prompt)}>
              <span class="prompt-source">{prompt.source_session_label}</span>
              <span class="prompt-name">{prompt.name}</span>
            </button>
          </li>
        {/each}
      </ul>
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
  }
  .modal {
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: 8px;
    width: 480px;
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
  .status {
    color: var(--text-secondary);
    font-size: 13px;
  }
  .status.error {
    color: var(--status-error);
  }
  .prompt-list {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 50vh;
    overflow-y: auto;
  }
  .prompt-list li {
    border-bottom: 1px solid var(--border-default);
  }
  .prompt-list li:last-child {
    border-bottom: none;
  }
  .prompt-btn {
    width: 100%;
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 10px 8px;
    background: none;
    border: none;
    color: var(--text-primary);
    font-size: 13px;
    cursor: pointer;
    text-align: left;
    box-shadow: none;
  }
  .prompt-btn:hover,
  .prompt-btn.selected {
    background: var(--bg-hover);
    border-radius: 4px;
  }
  .prompt-source {
    color: var(--text-emphasis);
    font-weight: 500;
    white-space: nowrap;
    flex-shrink: 0;
    font-size: 11px;
  }
  .prompt-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
