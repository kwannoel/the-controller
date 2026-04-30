<script lang="ts">
  import { projects as projectsStore, type FocusTarget } from "$lib/stores";
  import { fromStore } from "svelte/store";
  import { daemonStore } from "../daemon/store.svelte";
  import { groupSessionsByProject } from "../daemon/grouping";

  let {
    currentFocus,
    onProjectFocus,
    onNewChat,
    onSelect,
    onSelectChat,
  }: {
    currentFocus: FocusTarget;
    onProjectFocus: (projectId: string) => void;
    onNewChat: (projectId: string) => void;
    onSelect: (sessionId: string) => void;
    onSelectChat?: (chatId: string) => void;
  } = $props();

  const projectsState = fromStore(projectsStore);
  const projectList = $derived(projectsState.current);
  const sessionsList = $derived([...daemonStore.sessions.values()]);
  const chatsList = $derived([...daemonStore.chats.values()]);
  const groups = $derived(groupSessionsByProject(projectList, sessionsList));
</script>

<div class="list">
  {#each projectList as p (p.id)}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div
      class="project-row"
      class:focus-target={currentFocus?.type === "project" && currentFocus.projectId === p.id}
      tabindex="0"
      data-project-id={p.id}
      onfocusin={(e: FocusEvent) => {
        if (e.target === e.currentTarget) onProjectFocus(p.id);
      }}
    >{p.name}</div>
    {#each groups.byProject.get(p.id) ?? [] as s (s.id)}
      <button
        class="session-row"
        class:focus-target={currentFocus?.type === "session" && currentFocus.sessionId === s.id}
        data-session-id={s.id}
        onclick={() => onSelect(s.id)}
      >
        <span class="status status-{s.status}"></span>
        <span class="label">{s.label}</span>
        <span class="agent">{s.agent}</span>
      </button>
    {/each}
    {#each chatsList.filter((chat) => chat.project_id === p.id) as chat (chat.id)}
      <button
        class="session-row"
        class:focus-target={currentFocus?.type === "chat" && currentFocus.chatId === chat.id}
        data-chat-id={chat.id}
        onclick={() => onSelectChat?.(chat.id)}
      >
        <span class="status status-running"></span>
        <span class="label">{chat.title}</span>
        <span class="agent">chat</span>
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
  .project-row.focus-target, .session-row.focus-target { outline: 2px solid var(--focus-ring); outline-offset: -2px; border-radius: 4px; }
  .status { width: 8px; height: 8px; border-radius: 50%; }
  .status-running { background: var(--status-idle); }
  .status-ended, .status-failed { background: var(--status-error); }
  .status-starting, .status-interrupted { background: var(--status-working); }
  .agent { margin-left: auto; font-size: 11px; opacity: 0.6; }
</style>
