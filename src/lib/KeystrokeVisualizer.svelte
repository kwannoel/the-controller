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
    background: rgba(30, 30, 46, 0.95);
    color: #cdd6f4;
    border: 1px solid #45475a;
    border-radius: 8px;
    padding: 6px 14px;
    font-size: 15px;
    font-family: "JetBrains Mono", "Fira Code", monospace;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.4);
    animation: pill-fade ease-out forwards;
  }

  @keyframes pill-fade {
    0% { opacity: 1; }
    70% { opacity: 1; }
    100% { opacity: 0; }
  }
</style>
