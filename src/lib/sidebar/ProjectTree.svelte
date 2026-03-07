<script lang="ts">
  import type { FocusTarget, JumpPhase, Project, SessionStatus } from "../stores";

  interface Props {
    projects: Project[];
    mode: "active" | "archived";
    expandedProjectSet: Set<string>;
    activeSession: string | null;
    currentFocus: FocusTarget;
    jumpState: JumpPhase;
    projectJumpLabels: string[];
    getSessionStatus: (sessionId: string) => SessionStatus;
    onToggleProject: (projectId: string) => void;
    onProjectFocus: (projectId: string) => void;
    onSessionFocus: (sessionId: string, projectId: string) => void;
    onSessionSelect: (sessionId: string, projectId: string) => void;
  }

  let {
    projects,
    mode,
    expandedProjectSet,
    activeSession,
    currentFocus,
    jumpState,
    projectJumpLabels,
    getSessionStatus,
    onToggleProject,
    onProjectFocus,
    onSessionFocus,
    onSessionSelect,
  }: Props = $props();

  function isArchivedMode() {
    return mode === "archived";
  }
</script>

{#if isArchivedMode() && projects.length === 0}
  <div class="empty-state">No archived sessions</div>
{:else}
  {#each projects as project, i (project.id)}
    {@const visibleSessions = isArchivedMode()
      ? project.sessions.filter((s) => s.archived)
      : project.sessions.filter((s) => !s.archived)}
    <div class="project-item">
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="project-header"
        class:focus-target={currentFocus?.type === "project" && currentFocus.projectId === project.id}
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
        {#if jumpState?.phase === "project" && projectJumpLabels[i]}
          <kbd class="jump-label">{projectJumpLabels[i]}</kbd>
        {/if}
        <span class="session-count">{visibleSessions.length}</span>
      </div>

      {#if expandedProjectSet.has(project.id)}
        <div class="session-list">
          {#each visibleSessions as session (session.id)}
            {#if isArchivedMode()}
              <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
              <div
                class="session-item archived"
                class:focus-target={currentFocus?.type === "session" && currentFocus.sessionId === session.id}
                data-session-id={session.id}
                tabindex="0"
                onfocusin={() => {
                  onSessionFocus(session.id, project.id);
                }}
              >
                <span class="status-dot archived-dot">&cir;</span>
                <span class="session-label">{session.label}</span>
              </div>
            {:else}
              <div
                class="session-item"
                class:active={activeSession === session.id}
                class:focus-target={currentFocus?.type === "session" && currentFocus.sessionId === session.id}
                data-session-id={session.id}
                role="button"
                tabindex="0"
                onclick={() => {
                  onSessionSelect(session.id, project.id);
                  onSessionFocus(session.id, project.id);
                }}
                onfocusin={() => {
                  onSessionFocus(session.id, project.id);
                }}
                onkeydown={(e: KeyboardEvent) => {
                  if (e.key === "Enter" || e.key === " ") onSessionSelect(session.id, project.id);
                }}
              >
                <span
                  class="status-dot"
                  class:idle={getSessionStatus(session.id) === "idle"}
                  class:working={getSessionStatus(session.id) === "working"}
                >
                  {getSessionStatus(session.id) === "exited" ? "\u25CB" : "\u25CF"}
                </span>
                <span class="session-label">{session.label}</span>
                {#if session.github_issue}
                  <span class="issue-badge">#{session.github_issue.number}</span>
                {/if}
              </div>
            {/if}
          {/each}
        </div>
      {/if}
    </div>
  {/each}
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

  .session-count {
    font-size: 11px;
    color: #6c7086;
    background: #313244;
    padding: 1px 6px;
    border-radius: 8px;
  }

  .session-list {
    padding: 0;
  }

  .session-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 16px 6px 40px;
    cursor: pointer;
    font-size: 12px;
    width: 100%;
    background: none;
    border: none;
    color: #cdd6f4;
    text-align: left;
    box-shadow: none;
  }

  .session-item:hover {
    background: #313244;
  }

  .session-item.active {
    background: #45475a;
  }

  .session-item.focus-target {
    outline: 2px solid #89b4fa;
    outline-offset: -2px;
    border-radius: 4px;
  }

  .session-item.archived {
    opacity: 0.6;
  }

  .archived-dot {
    color: #6c7086;
  }

  .status-dot {
    font-size: 10px;
    color: #6c7086;
  }

  .status-dot.idle {
    color: #a6e3a1;
  }

  .status-dot.working {
    color: #f9e2af;
  }

  .session-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
  }

  .issue-badge {
    font-size: 10px;
    color: #89b4fa;
    background: rgba(137, 180, 250, 0.15);
    padding: 0 4px;
    border-radius: 3px;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .jump-label {
    background: #fab387;
    color: #1e1e2e;
    padding: 0 5px;
    border-radius: 3px;
    font-family: monospace;
    font-size: 11px;
    font-weight: 700;
    line-height: 16px;
    flex-shrink: 0;
    margin-left: auto;
  }

  .empty-state {
    padding: 24px 16px;
    color: #6c7086;
    font-size: 13px;
    text-align: center;
  }
</style>
