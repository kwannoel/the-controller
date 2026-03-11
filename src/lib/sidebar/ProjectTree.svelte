<script lang="ts">
  import type { FocusTarget, Project, SessionStatus } from "../stores";

  interface Props {
    projects: Project[];
    expandedProjectSet: Set<string>;
    activeSession: string | null;
    currentFocus: FocusTarget;
    getSessionStatus: (sessionId: string) => SessionStatus;
    onToggleProject: (projectId: string) => void;
    onProjectFocus: (projectId: string) => void;
    onSessionFocus: (sessionId: string, projectId: string) => void;
    onSessionSelect: (sessionId: string, projectId: string) => void;
  }

  let {
    projects,
    expandedProjectSet,
    activeSession,
    currentFocus,
    getSessionStatus,
    onToggleProject,
    onProjectFocus,
    onSessionFocus,
    onSessionSelect,
  }: Props = $props();
</script>

{#each projects as project (project.id)}
  {@const visibleSessions = project.sessions.filter((s) => !s.auto_worker_session)}
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
      <span class="session-count">{visibleSessions.length}</span>
    </div>

    {#if expandedProjectSet.has(project.id)}
      <div class="session-list">
        {#each visibleSessions as session (session.id)}
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
            {#if project.staged_session?.session_id === session.id}
              <span class="staged-badge">staged</span>
            {/if}
            {#if session.github_issue}
              <span class="issue-badge">#{session.github_issue.number}</span>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>
{/each}

<style>
  .project-item {
    border-bottom: 1px solid var(--border-default);
  }

  .project-header {
    display: flex;
    align-items: center;
    padding: 8px 16px;
    gap: 8px;
  }

  .project-header:hover {
    background: var(--bg-hover);
  }

  .project-header.focus-target {
    outline: 2px solid var(--focus-ring);
    outline-offset: -2px;
    border-radius: 4px;
  }

  .btn-expand {
    background: none;
    border: none;
    color: var(--text-secondary);
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
    color: var(--text-secondary);
    background: var(--bg-hover);
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
    color: var(--text-primary);
    text-align: left;
    box-shadow: none;
  }

  .session-item:hover {
    background: var(--bg-hover);
  }

  .session-item.active {
    background: var(--bg-active);
  }

  .session-item.focus-target {
    outline: 2px solid var(--focus-ring);
    outline-offset: -2px;
    border-radius: 4px;
  }

  .status-dot {
    font-size: 10px;
    color: var(--text-secondary);
  }

  .status-dot.idle {
    color: var(--status-idle);
  }

  .status-dot.working {
    color: var(--status-working);
  }

  .session-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
  }

  .staged-badge {
    font-size: 9px;
    color: var(--status-error);
    background: rgba(196, 64, 64, 0.15);
    padding: 0 4px;
    border-radius: 3px;
    white-space: nowrap;
    flex-shrink: 0;
    text-transform: uppercase;
    font-weight: 600;
    letter-spacing: 0.5px;
  }

  .issue-badge {
    font-size: 10px;
    color: var(--text-emphasis);
    background: rgba(255, 255, 255, 0.08);
    padding: 0 4px;
    border-radius: 3px;
    white-space: nowrap;
    flex-shrink: 0;
  }
</style>
