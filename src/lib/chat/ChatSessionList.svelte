<script lang="ts">
  import { projects as projectsStore } from "$lib/stores";
  import { fromStore } from "svelte/store";
  import { daemonStore } from "../daemon/store.svelte";
  import { groupSessionsByProject } from "../daemon/grouping";

  let { onNewChat, onSelect }: { onNewChat: (projectId: string) => void; onSelect: (sessionId: string) => void } = $props();

  const projectsState = fromStore(projectsStore);
  const projectList = $derived(projectsState.current);
  const sessionsList = $derived([...daemonStore.sessions.values()]);
  const groups = $derived(groupSessionsByProject(projectList, sessionsList));
</script>

<div class="list">
  {#each projectList as p (p.id)}
    <div class="project-row">{p.name}</div>
    {#each groups.byProject.get(p.id) ?? [] as s (s.id)}
      <button class="session-row" onclick={() => onSelect(s.id)}>
        <span class="status status-{s.status}"></span>
        <span class="label">{s.label}</span>
        <span class="agent">{s.agent}</span>
      </button>
    {/each}
    <button class="new" onclick={() => onNewChat(p.id)}>+ New chat</button>
  {/each}
  {#if groups.other.length}
    <div class="project-row">Other</div>
    {#each groups.other as s (s.id)}
      <button class="session-row" onclick={() => onSelect(s.id)}>
        <span class="status status-{s.status}"></span>
        <span class="label">{s.label}</span>
        <span class="agent">{s.agent}</span>
      </button>
    {/each}
  {/if}
</div>

<style>
  .list { display: flex; flex-direction: column; gap: 4px; padding: 8px; }
  .project-row { font-weight: 600; font-size: 13px; padding: 4px 0; }
  .session-row, .new { background: transparent; border: 0; color: inherit; text-align: left; padding: 4px 8px; cursor: pointer; display: flex; gap: 8px; align-items: center; }
  .session-row:hover, .new:hover { background: rgba(255,255,255,0.05); }
  .status { width: 8px; height: 8px; border-radius: 50%; }
  .status-running { background: #a6e3a1; }
  .status-ended, .status-failed { background: #f38ba8; }
  .status-starting, .status-interrupted { background: #f9e2af; }
  .agent { margin-left: auto; font-size: 11px; opacity: 0.6; }
</style>
