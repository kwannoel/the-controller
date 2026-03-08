<script lang="ts">
  import { fromStore } from "svelte/store";
  import { workspaceMode, type WorkspaceMode } from "./stores";

  const workspaceModeState = fromStore(workspaceMode);
  let currentMode: WorkspaceMode = $derived(workspaceModeState.current);

  const modes: { key: string; id: WorkspaceMode; label: string }[] = [
    { key: "d", id: "development", label: "Development" },
    { key: "a", id: "agents", label: "Agents" },
  ];
</script>

<div class="overlay">
  <div class="picker">
    <div class="picker-title">Switch Workspace</div>
    <div class="picker-options">
      {#each modes as mode}
        <div class="picker-option" class:active={currentMode === mode.id}>
          <kbd>{mode.key}</kbd>
          <span class="option-label">{mode.label}</span>
          {#if currentMode === mode.id}
            <span class="current-badge">current</span>
          {/if}
        </div>
      {/each}
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .picker {
    background: #1e1e2e;
    border: 1px solid #313244;
    border-radius: 8px;
    padding: 20px 24px;
    min-width: 240px;
  }

  .picker-title {
    font-size: 14px;
    font-weight: 600;
    color: #cdd6f4;
    margin-bottom: 16px;
    text-align: center;
  }

  .picker-options {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .picker-option {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    border-radius: 6px;
    color: #a6adc8;
  }

  .picker-option.active {
    background: rgba(137, 180, 250, 0.1);
    color: #cdd6f4;
  }

  kbd {
    background: #ffffff;
    color: #1e1e2e;
    padding: 2px 8px;
    border-radius: 4px;
    font-family: monospace;
    font-size: 13px;
    font-weight: 600;
  }

  .option-label {
    flex: 1;
    font-size: 13px;
  }

  .current-badge {
    font-size: 11px;
    color: #89b4fa;
    font-style: italic;
  }
</style>
