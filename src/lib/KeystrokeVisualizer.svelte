<!-- src/lib/KeystrokeVisualizer.svelte -->
<script lang="ts">
  import { fromStore } from "svelte/store";
  import { keystrokeVisualizerEnabled, keystrokes } from "./keystroke-visualizer";

  const enabledState = fromStore(keystrokeVisualizerEnabled);
  const keystrokesState = fromStore(keystrokes);
  let enabled = $derived(enabledState.current);
  let list = $derived(keystrokesState.current);
</script>

{#if enabled && list.length > 0}
  <div class="keystroke-container">
    {#each list as ks (ks.id)}
      <span class="keystroke-pill">{ks.label}</span>
    {/each}
  </div>
{/if}

<style>
  .keystroke-container {
    position: fixed;
    bottom: 16px;
    left: 16px;
    z-index: 1000;
    display: flex;
    flex-direction: row;
    gap: 6px;
    pointer-events: none;
  }

  .keystroke-pill {
    background: rgba(30, 30, 46, 0.85);
    color: #cdd6f4;
    border: 1px solid #313244;
    border-radius: 6px;
    padding: 4px 10px;
    font-size: 13px;
    font-family: "JetBrains Mono", "Fira Code", monospace;
    animation: pill-fade 2s ease-out forwards;
  }

  @keyframes pill-fade {
    0% { opacity: 1; }
    70% { opacity: 1; }
    100% { opacity: 0; }
  }
</style>
