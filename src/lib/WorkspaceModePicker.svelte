<script lang="ts">
  import { fromStore } from "svelte/store";
  import { workspaceMode, type WorkspaceMode } from "./stores";

  const workspaceModeState = fromStore(workspaceMode);
  let currentMode: WorkspaceMode = $derived(workspaceModeState.current);

  const modes: { key: string; id: WorkspaceMode; label: string }[] = [
    { key: "d", id: "development", label: "Development" },
    { key: "a", id: "agents", label: "Agents" },
    { key: "r", id: "architecture", label: "Architecture" },
    { key: "n", id: "notes", label: "Notes" },
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
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(16px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .picker {
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: 8px;
    padding: 20px 24px;
    min-width: 240px;
  }

  .picker-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary);
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
    color: var(--text-secondary);
  }

  .picker-option.active {
    background: rgba(255, 255, 255, 0.05);
    color: var(--text-primary);
  }

  kbd {
    background: var(--text-emphasis);
    color: var(--bg-void);
    padding: 2px 8px;
    border-radius: 4px;
    font-family: var(--font-mono);
    font-size: 13px;
    font-weight: 600;
  }

  .option-label {
    flex: 1;
    font-size: 13px;
  }

  .current-badge {
    font-size: 11px;
    color: var(--text-emphasis);
    font-style: italic;
  }
</style>
