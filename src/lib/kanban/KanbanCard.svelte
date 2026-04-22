<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import type { GithubIssue } from "../stores";
  import { isStatusLabel } from "./columns";

  interface Props {
    issue: GithubIssue;
    ondragstart?: (issue: GithubIssue) => void;
    ondragend?: () => void;
  }

  let { issue, ondragstart, ondragend }: Props = $props();

  let nonStatusLabels = $derived(
    issue.labels.filter((l) => !isStatusLabel(l.name)),
  );

  function avatarUrl(login: string, explicit?: string): string {
    return explicit ?? `https://github.com/${login}.png`;
  }

  function onClick() {
    openUrl(issue.url).catch(() => {});
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onClick();
    }
  }

  function onDragStart(e: DragEvent) {
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = "move";
      e.dataTransfer.setData("text/plain", String(issue.number));
    }
    ondragstart?.(issue);
  }

  function onDragEnd() {
    ondragend?.();
  }
</script>

<div
  class="kanban-card"
  draggable="true"
  role="button"
  tabindex="0"
  onclick={onClick}
  onkeydown={onKeydown}
  ondragstart={onDragStart}
  ondragend={onDragEnd}
  data-issue-number={issue.number}
>
  <header class="card-header">
    <span class="card-number">#{issue.number}</span>
    {#if issue.assignees && issue.assignees.length > 0}
      <div class="avatars">
        {#each issue.assignees.slice(0, 3) as a (a.login)}
          <img
            class="avatar"
            src={avatarUrl(a.login, a.avatarUrl)}
            alt={a.login}
            title={a.login}
          />
        {/each}
      </div>
    {/if}
  </header>
  <p class="card-title">{issue.title}</p>
  {#if nonStatusLabels.length > 0}
    <div class="card-labels">
      {#each nonStatusLabels as label (label.name)}
        <span class="label-chip">{label.name}</span>
      {/each}
    </div>
  {/if}
</div>

<style>
  .kanban-card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px;
    background: var(--bg-void);
    border: 1px solid var(--border-default);
    border-radius: 6px;
    cursor: grab;
    color: var(--text-primary);
    text-align: left;
    user-select: none;
  }

  .kanban-card:hover {
    border-color: var(--text-emphasis);
  }

  .kanban-card:active {
    cursor: grabbing;
  }

  .card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .card-number {
    font-size: 11px;
    color: var(--text-secondary);
    font-family: var(--font-mono);
  }

  .card-title {
    margin: 0;
    font-size: 13px;
    line-height: 1.35;
  }

  .avatars {
    display: inline-flex;
    gap: 2px;
  }

  .avatar {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--bg-elevated);
  }

  .card-labels {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .label-chip {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 10px;
    background: var(--bg-elevated);
    color: var(--text-secondary);
    border: 1px solid var(--border-default);
  }
</style>
