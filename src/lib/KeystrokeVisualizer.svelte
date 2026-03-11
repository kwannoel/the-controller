<!-- src/lib/KeystrokeVisualizer.svelte -->
<script lang="ts">
  import { fromStore } from "svelte/store";
  import { keystrokeVisualizerEnabled, keystrokes, FADE_MS } from "./keystroke-visualizer";

  const enabledState = fromStore(keystrokeVisualizerEnabled);
  const keystrokesState = fromStore(keystrokes);
  let enabled = $derived(enabledState.current);
  let list = $derived(keystrokesState.current);
</script>

{#if enabled && list.length > 0}
  <div class="keystroke-container">
    {#each list as ks (ks.id)}
      <span class="keystroke-pill" style="animation-duration: {FADE_MS}ms">{ks.label}</span>
    {/each}
  </div>
{/if}

<style>
  .keystroke-container {
    position: fixed;
    bottom: 12px;
    right: 12px;
    z-index: 1000;
    display: flex;
    flex-direction: row;
    gap: 6px;
    pointer-events: none;
  }

  .keystroke-pill {
    background: rgba(28, 28, 28, 0.95);
    color: var(--text-primary);
    border: 1px solid var(--border-default);
    border-radius: 8px;
    padding: 6px 14px;
    font-size: 15px;
    font-family: var(--font-mono);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.4);
    animation: pill-fade ease-out forwards;
  }

  @keyframes pill-fade {
    0% { opacity: 1; }
    70% { opacity: 1; }
    100% { opacity: 0; }
  }
</style>
