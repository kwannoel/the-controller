<script lang="ts">
  import { fromStore } from "svelte/store";
  import { autoWorkerStatuses, type Project, type FocusTarget, type AutoWorkerStatus } from "../stores";

  interface Props {
    projects: Project[];
    currentFocus: FocusTarget;
    onProjectFocus: (projectId: string) => void;
  }

  let { projects, currentFocus, onProjectFocus }: Props = $props();

  const autoWorkerStatusesState = fromStore(autoWorkerStatuses);
  let statusMap: Map<string, AutoWorkerStatus> = $derived(autoWorkerStatusesState.current);

  function getAgentStatus(projectId: string): AutoWorkerStatus | null {
    return statusMap.get(projectId) ?? null;
  }

  function isProjectFocused(projectId: string): boolean {
    return currentFocus?.type === "project" && currentFocus.projectId === projectId;
  }
</script>

{#each projects as project (project.id)}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="agent-project"
    class:focused={isProjectFocused(project.id)}
    data-project-id={project.id}
    tabindex="0"
    onfocus={() => onProjectFocus(project.id)}
  >
    <div class="project-header">
      <span class="project-name">{project.name}</span>
      <span class="agent-badge" class:enabled={project.auto_worker.enabled}>
        {project.auto_worker.enabled ? "ON" : "OFF"}
      </span>
    </div>
    <div class="agent-status">
      {#if !project.auto_worker.enabled}
        <span class="status-text muted">Agent disabled</span>
      {:else}
        {@const status = getAgentStatus(project.id)}
        {#if status?.status === "working"}
          <span class="status-dot working"></span>
          <span class="status-text">#{status.issue_number} {status.issue_title}</span>
        {:else}
          <span class="status-dot idle"></span>
          <span class="status-text muted">Waiting for issues</span>
        {/if}
      {/if}
    </div>
    {#if project.maintainer.enabled}
      <div class="maintainer-badge">
        <span class="maintainer-label">Maintainer ON</span>
      </div>
    {/if}
  </div>
{/each}

{#if projects.length === 0}
  <div class="empty">No projects</div>
{/if}

<style>
  .agent-project {
    padding: 10px 16px;
    border-bottom: 1px solid #313244;
    cursor: pointer;
    outline: none;
  }
  .agent-project:hover { background: rgba(49, 50, 68, 0.5); }
  .agent-project.focused { background: #313244; }
  .project-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 4px;
  }
  .project-name { font-size: 13px; font-weight: 500; color: #cdd6f4; }
  .agent-badge {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 3px;
    background: #313244;
    color: #6c7086;
  }
  .agent-badge.enabled { background: rgba(166, 227, 161, 0.2); color: #a6e3a1; }
  .agent-status { display: flex; align-items: center; gap: 6px; font-size: 12px; }
  .status-dot { width: 6px; height: 6px; border-radius: 50%; flex-shrink: 0; }
  .status-dot.working { background: #f9e2af; }
  .status-dot.idle { background: #a6e3a1; }
  .status-text { color: #cdd6f4; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .status-text.muted { color: #6c7086; }
  .maintainer-badge { margin-top: 4px; }
  .maintainer-label { font-size: 10px; color: #89b4fa; }
  .empty { padding: 16px; color: #6c7086; font-size: 13px; text-align: center; }
</style>
