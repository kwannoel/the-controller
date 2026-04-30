<script lang="ts">
  export interface SummaryAgent {
    id: string;
    handle: string;
    name: string;
    focused?: boolean;
  }

  export interface SummaryWorkspace {
    id: string;
    label: string;
    path: string;
    focused?: boolean;
  }

  let {
    agents = [],
    workspaces = [],
  }: {
    agents?: SummaryAgent[];
    workspaces?: SummaryWorkspace[];
  } = $props();

  const focusedAgent = $derived(agents.find((agent) => agent.focused) ?? null);
  const focusedWorkspace = $derived(workspaces.find((workspace) => workspace.focused) ?? null);
  const isEmpty = $derived(agents.length === 0 && workspaces.length === 0);
</script>

<section class="summary" aria-label="Chat summary">
  {#if isEmpty}
    <span class="none">none</span>
  {:else}
    <div class="group">
      <span class="label">agents</span>
      {#each agents as agent (agent.id)}
        <span class:focused={agent.focused} class="chip" title={agent.name}>@{agent.handle}</span>
      {/each}
      {#if focusedAgent}
        <span class="focus">focused @{focusedAgent.handle}</span>
      {/if}
    </div>

    <div class="group">
      <span class="label">workspaces</span>
      {#each workspaces as workspace (workspace.id)}
        <span class:focused={workspace.focused} class="chip" title={workspace.path}>{workspace.label}</span>
      {/each}
      {#if focusedWorkspace}
        <span class="focus">focused {focusedWorkspace.label}</span>
      {/if}
    </div>
  {/if}
</section>

<style>
  .summary {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 12px;
    align-items: center;
    min-height: 30px;
    padding: 5px 12px;
    border-bottom: 1px solid var(--border-default);
    color: var(--text-secondary);
    font-size: 11px;
  }

  .group {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    align-items: center;
    min-width: 0;
  }

  .label {
    opacity: 0.7;
  }

  .chip,
  .focus,
  .none {
    border: 1px solid var(--border-default);
    border-radius: 4px;
    padding: 1px 5px;
    background: var(--bg-elevated);
    line-height: 16px;
  }

  .chip.focused,
  .focus {
    color: var(--text-primary);
    border-color: var(--text-secondary);
  }

  .none {
    opacity: 0.75;
  }
</style>
