<script lang="ts">
  import { onMount } from "svelte";
  import type { TriageCategory } from "./stores";

  interface Props {
    onSelect: (category: TriageCategory) => void;
    onClose: () => void;
  }

  let { onSelect, onClose }: Props = $props();

  const categories: { key: string; value: TriageCategory; label: string; color: string }[] = [
    { key: "1", value: "untagged", label: "Untagged", color: "#89b4fa" },
    { key: "2", value: "high", label: "High Priority", color: "#f38ba8" },
    { key: "3", value: "low", label: "Low Priority", color: "#a6e3a1" },
  ];

  let selectedIndex = $state(0);

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
      return;
    }

    if (e.key === "ArrowDown" || e.key === "j") {
      e.preventDefault();
      e.stopPropagation();
      selectedIndex = (selectedIndex + 1) % categories.length;
    } else if (e.key === "ArrowUp" || e.key === "k") {
      e.preventDefault();
      e.stopPropagation();
      selectedIndex = (selectedIndex - 1 + categories.length) % categories.length;
    } else if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      onSelect(categories[selectedIndex].value);
    } else if (e.key === "1" || e.key === "2" || e.key === "3") {
      e.preventDefault();
      e.stopPropagation();
      const cat = categories.find(c => c.key === e.key);
      if (cat) onSelect(cat.value);
    }
  }

  onMount(() => {
    window.addEventListener("keydown", handleKeydown, { capture: true });
    return () => {
      window.removeEventListener("keydown", handleKeydown, { capture: true });
    };
  });
</script>

<div class="overlay" onclick={onClose} role="dialog">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="picker" onclick={(e) => e.stopPropagation()} role="presentation">
    <div class="picker-header">Triage Category</div>
    <div class="picker-list">
      {#each categories as cat, i}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          class="picker-item"
          class:selected={i === selectedIndex}
          onclick={() => onSelect(cat.value)}
          role="option"
          aria-selected={i === selectedIndex}
        >
          <span class="picker-key">{cat.key}</span>
          <span class="picker-dot" style="background: {cat.color}"></span>
          <span class="picker-label">{cat.label}</span>
        </div>
      {/each}
    </div>
    <div class="picker-hint">
      <kbd>j</kbd>/<kbd>k</kbd> navigate · <kbd>Enter</kbd> select · <kbd>1-3</kbd> jump
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .picker {
    background: #1e1e2e;
    border: 1px solid #313244;
    border-radius: 8px;
    width: 280px;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .picker-header {
    font-size: 14px;
    font-weight: 600;
    color: #cdd6f4;
  }

  .picker-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .picker-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: 6px;
    cursor: pointer;
    color: #cdd6f4;
    font-size: 13px;
  }

  .picker-item:hover,
  .picker-item.selected {
    background: #313244;
  }

  .picker-key {
    color: #89b4fa;
    font-family: monospace;
    font-size: 12px;
    font-weight: 600;
    width: 16px;
    text-align: center;
  }

  .picker-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .picker-label {
    flex: 1;
  }

  .picker-hint {
    color: #6c7086;
    font-size: 11px;
    text-align: center;
    padding-top: 4px;
    border-top: 1px solid #313244;
  }

  kbd {
    background: #313244;
    color: #89b4fa;
    padding: 1px 5px;
    border-radius: 3px;
    font-family: monospace;
    font-size: 11px;
  }
</style>
