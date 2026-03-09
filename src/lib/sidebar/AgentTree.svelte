<script lang="ts">
  import { fromStore } from "svelte/store";
  import { autoWorkerStatuses, maintainerStatuses, type AgentKind, type Project, type FocusTarget, type AutoWorkerStatus, type MaintainerStatus } from "../stores";

  interface Props {
    projects: Project[];
    expandedProjectSet: Set<string>;
    currentFocus: FocusTarget;
    onToggleProject: (projectId: string) => void;
    onProjectFocus: (projectId: string) => void;
    onAgentFocus: (agentKind: AgentKind, projectId: string) => void;
  }

  let { projects, expandedProjectSet, currentFocus, onToggleProject, onProjectFocus, onAgentFocus }: Props = $props();

  const autoWorkerStatusesState = fromStore(autoWorkerStatuses);
  let awStatusMap: Map<string, AutoWorkerStatus> = $derived(autoWorkerStatusesState.current);

  const maintainerStatusesState = fromStore(maintainerStatuses);
  let mStatusMap: Map<string, MaintainerStatus> = $derived(maintainerStatusesState.current);

  function isProjectFocused(projectId: string): boolean {
    return currentFocus?.type === "project" && currentFocus.projectId === projectId;
  }

  function isAgentFocused(projectId: string, kind: AgentKind): boolean {
    if (!currentFocus) return false;
    return currentFocus.type === "agent" && currentFocus.projectId === projectId && currentFocus.agentKind === kind;
  }

  function isAgentActive(projectId: string, kind: AgentKind): boolean {
    if (!currentFocus) return false;
    return currentFocus.type === "agent-panel" && currentFocus.projectId === projectId && currentFocus.agentKind === kind;
  }

  function awIsWorking(projectId: string): boolean {
    return awStatusMap.get(projectId)?.status === "working";
  }

  function mStatusValue(projectId: string): MaintainerStatus | null {
    return mStatusMap.get(projectId) ?? null;
  }
</script>

{#each projects as project (project.id)}
  <div class="project-item">
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="project-header"
      class:focus-target={isProjectFocused(project.id)}
      tabindex="0"
      data-project-id={project.id}
      onfocusin={(e: FocusEvent) => {
        if (e.target === e.currentTarget) onProjectFocus(project.id);
      }}
    >
      <button class="btn-expand" onclick={() => onToggleProject(project.id)}>
        {expandedProjectSet.has(project.id) ? "\u25BC" : "\u25B6"}
      </button>
      <span class="project-name">{project.name}</span>
      <span class="agent-count">2</span>
    </div>

    {#if expandedProjectSet.has(project.id)}
      <div class="agent-list">
        <!-- Auto-worker -->
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <div
          class="agent-item"
          class:focus-target={isAgentFocused(project.id, "auto-worker")}
          class:active={isAgentActive(project.id, "auto-worker")}
          data-agent-id="{project.id}:auto-worker"
          tabindex="0"
          onfocusin={() => onAgentFocus("auto-worker", project.id)}
        >
          <span class="status-dot" class:working={awIsWorking(project.id)} class:idle={project.auto_worker.enabled && !awIsWorking(project.id)} class:disabled={!project.auto_worker.enabled}></span>
          <span class="agent-label">Auto-worker</span>
          <span class="agent-badge" class:enabled={project.auto_worker.enabled}>
            {project.auto_worker.enabled ? "ON" : "OFF"}
          </span>
        </div>

        <!-- Maintainer -->
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <div
          class="agent-item"
          class:focus-target={isAgentFocused(project.id, "maintainer")}
          class:active={isAgentActive(project.id, "maintainer")}
          data-agent-id="{project.id}:maintainer"
          tabindex="0"
          onfocusin={() => onAgentFocus("maintainer", project.id)}
        >
          <span class="status-dot" class:working={mStatusValue(project.id) === "running"} class:error={mStatusValue(project.id) === "error"} class:idle={project.maintainer.enabled && mStatusValue(project.id) !== "running" && mStatusValue(project.id) !== "error"} class:disabled={!project.maintainer.enabled}></span>
          <span class="agent-label">Maintainer</span>
          <span class="agent-badge" class:enabled={project.maintainer.enabled}>
            {project.maintainer.enabled ? "ON" : "OFF"}
          </span>
        </div>
      </div>
    {/if}
  </div>
{/each}

{#if projects.length === 0}
  <div class="empty">No projects</div>
{/if}

<style>
  .project-item {
    border-bottom: 1px solid #313244;
  }

  .project-header {
    display: flex;
    align-items: center;
    padding: 8px 16px;
    gap: 8px;
  }

  .project-header:hover {
    background: #313244;
  }

  .project-header.focus-target {
    outline: 2px solid #89b4fa;
    outline-offset: -2px;
    border-radius: 4px;
  }

  .btn-expand {
    background: none;
    border: none;
    color: #6c7086;
    cursor: pointer;
    padding: 0;
    font-size: 10px;
    width: 16px;
    text-align: center;
    box-shadow: none;
  }

  .project-name {
    flex: 1;
    font-size: 13px;
    font-weight: 500;
    word-break: break-word;
  }

  .agent-count {
    font-size: 11px;
    color: #6c7086;
    background: #313244;
    padding: 1px 6px;
    border-radius: 8px;
  }

  .agent-list {
    padding: 0;
  }

  .agent-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 16px 6px 40px;
    cursor: pointer;
    font-size: 12px;
    outline: none;
  }

  .agent-item:hover {
    background: #313244;
  }

  .agent-item.focus-target {
    outline: 2px solid #89b4fa;
    outline-offset: -2px;
    border-radius: 4px;
  }

  .agent-item.active {
    background: rgba(137, 180, 250, 0.1);
  }

  .status-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
    background: #6c7086;
  }

  .status-dot.working { background: #f9e2af; }
  .status-dot.idle { background: #a6e3a1; }
  .status-dot.error { background: #f38ba8; }
  .status-dot.disabled { background: #6c7086; }

  .agent-label {
    flex: 1;
    color: #cdd6f4;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .agent-badge {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 3px;
    background: #313244;
    color: #6c7086;
    flex-shrink: 0;
  }

  .agent-badge.enabled { background: rgba(166, 227, 161, 0.2); color: #a6e3a1; }

  .empty { padding: 16px; color: #6c7086; font-size: 13px; text-align: center; }
</style>
