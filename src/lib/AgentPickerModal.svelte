<script lang="ts">
  import { onMount } from "svelte";
  import { command } from "$lib/backend";

  interface AgentEntry {
    name: string;
    title: string;
  }

  interface Props {
    projectId: string;
    onSelect: (agentName: string) => void;
    onCancel: () => void;
  }

  let { projectId, onSelect, onCancel }: Props = $props();

  let agents: AgentEntry[] = $state([]);
  let selectedIndex = $state(0);
  let loading = $state(true);
  let error: string | null = $state(null);

  onMount(async () => {
    try {
      agents = await command<AgentEntry[]>("list_agents", { projectId });
      loading = false;
    } catch (e) {
      error = String(e);
      loading = false;
    }
  });

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onCancel();
    } else if (e.key === "j" || e.key === "ArrowDown") {
      e.preventDefault();
      e.stopPropagation();
      if (agents.length > 0) selectedIndex = (selectedIndex + 1) % agents.length;
    } else if (e.key === "k" || e.key === "ArrowUp") {
      e.preventDefault();
      e.stopPropagation();
      if (agents.length > 0) selectedIndex = (selectedIndex - 1 + agents.length) % agents.length;
    } else if (e.key === "Enter" || e.key === "l") {
      e.preventDefault();
      e.stopPropagation();
      if (agents.length > 0) onSelect(agents[selectedIndex].name);
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="overlay" role="dialog">
  <div class="picker">
    <div class="picker-title">Spawn Agent</div>
    {#if loading}
      <div class="state-msg">Loading agents...</div>
    {:else if error}
      <div class="state-msg error">{error}</div>
    {:else if agents.length === 0}
      <div class="state-msg">No agents found. Add agents to agents/ in your project.</div>
    {:else}
      <div class="agent-list">
        {#each agents as agent, i}
          <button
            type="button"
            class="agent-item"
            class:selected={i === selectedIndex}
            onclick={() => onSelect(agent.name)}
          >
            <span class="agent-name">{agent.title || agent.name}</span>
            <span class="agent-dir">{agent.name}/</span>
          </button>
        {/each}
      </div>
    {/if}
    <div class="hint">j/k navigate &middot; Enter select &middot; Esc cancel</div>
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
    min-width: 320px;
    max-width: 480px;
  }

  .picker-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary);
    margin-bottom: 16px;
    text-align: center;
  }

  .agent-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .agent-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px;
    border-radius: 6px;
    cursor: pointer;
    color: var(--text-secondary);
    background: none;
    border: none;
    font: inherit;
    width: 100%;
    text-align: left;
  }

  .agent-item.selected {
    background: rgba(255, 255, 255, 0.08);
    color: var(--text-primary);
  }

  .agent-name {
    font-size: 13px;
    font-weight: 500;
  }

  .agent-dir {
    font-size: 11px;
    color: var(--text-muted);
    font-family: var(--font-mono);
  }

  .hint {
    margin-top: 16px;
    font-size: 11px;
    color: var(--text-muted);
    text-align: center;
  }

  .state-msg {
    padding: 12px;
    text-align: center;
    font-size: 13px;
    color: var(--text-secondary);
  }

  .state-msg.error {
    color: var(--red);
  }
</style>
