<script lang="ts">
  import { command } from "$lib/backend";
  import { onMount } from "svelte";
  import { showToast } from "./toast";
  import type { DirEntry } from "./stores";

  interface Props {
    onSelect: (entry: DirEntry) => void;
    onClose: () => void;
  }

  let { onSelect, onClose }: Props = $props();

  let query = $state("");
  let entries = $state<DirEntry[]>([]);
  let filtered = $derived(
    query.trim() === ""
      ? entries
      : entries.filter((e) =>
          e.name.toLowerCase().includes(query.toLowerCase()),
        ),
  );
  let selectedIndex = $state(0);
  let inputEl: HTMLInputElement | undefined = $state();

  onMount(async () => {
    try {
      entries = await command<DirEntry[]>("list_root_directories");
    } catch (e) {
      showToast(String(e), "error");
    }
    inputEl?.focus();
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = Math.min(selectedIndex + 1, filtered.length - 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex = Math.max(selectedIndex - 1, 0);
    } else if (e.key === "Enter" && filtered.length > 0) {
      e.preventDefault();
      onSelect(filtered[selectedIndex]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    }
  }

  // Reset selection when query changes
  $effect(() => {
    query;
    selectedIndex = 0;
  });
</script>

<div class="overlay" onclick={onClose} role="dialog">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal" onclick={(e) => e.stopPropagation()} role="presentation">
    <input
      bind:this={inputEl}
      bind:value={query}
      placeholder="Search projects..."
      class="search-input"
      onkeydown={handleKeydown}
    />
    <div class="results">
      {#each filtered as entry, i (entry.path)}
        <div
          class="result-item"
          class:selected={i === selectedIndex}
          onclick={() => onSelect(entry)}
          role="option"
          aria-selected={i === selectedIndex}
        >
          <span class="entry-name">{entry.name}</span>
          <span class="entry-path">{entry.path}</span>
        </div>
      {/each}
      {#if filtered.length === 0}
        <div class="empty">No matching directories</div>
      {/if}
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
    width: 500px;
    max-height: 400px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .search-input {
    background: var(--bg-elevated);
    color: var(--text-primary);
    border: none;
    border-bottom: 1px solid var(--border-default);
    padding: 14px 16px;
    font-size: 15px;
    outline: none;
  }
  .results {
    overflow-y: auto;
    max-height: 300px;
  }
  .result-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 16px;
    cursor: pointer;
  }
  .result-item:hover,
  .result-item.selected {
    background: var(--bg-hover);
  }
  .entry-name {
    color: var(--text-primary);
    font-size: 14px;
  }
  .entry-path {
    color: var(--text-secondary);
    font-size: 12px;
  }
  .empty {
    padding: 20px 16px;
    color: var(--text-secondary);
    font-size: 13px;
    text-align: center;
  }
</style>
