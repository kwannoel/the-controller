<script lang="ts">
  import { fromStore } from "svelte/store";
  import { listen } from "$lib/backend";
  import { refreshProjectsFromBackend } from "./project-listing";
  import {
    projects,
    maintainerStatuses,
    maintainerErrors,
    autoWorkerStatuses,
    showKeyHints,
    focusTarget,
    expandedProjects,
    workspaceMode,
    type CorruptProjectEntry,
    type Project,
    type FocusTarget,
    type MaintainerStatus,
  } from "./stores";
  import { showToast } from "./toast";
  import AgentTree from "./sidebar/AgentTree.svelte";
  import ProjectList from "./sidebar/ProjectList.svelte";
  import ChatSessionList from "./chat/ChatSessionList.svelte";
  import { daemonStore } from "./daemon/store.svelte";

  let sidebarEl: HTMLElement | undefined = $state();
  const showKeyHintsState = fromStore(showKeyHints);
  const expandedProjectsState = fromStore(expandedProjects);
  let expandedProjectSet: Set<string> = $derived(expandedProjectsState.current);
  const workspaceModeState = fromStore(workspaceMode);
  let currentMode = $derived(workspaceModeState.current);
  const projectsState = fromStore(projects);
  let projectList: Project[] = $derived(projectsState.current);
  let surfacedCorruptProjectWarnings = $state(new Set<string>());

  const focusTargetState = fromStore(focusTarget);
  let currentFocus: FocusTarget = $derived(focusTargetState.current);

  let modeTitle = $derived(
    currentMode === "agents" ? "Agents" :
    currentMode === "kanban" ? "Kanban" : "Chat",
  );

  $effect(() => {
    if (!sidebarEl) return;

    if (currentFocus?.type === "project") {
      requestAnimationFrame(() => {
        sidebarEl?.querySelector<HTMLElement>(`[data-project-id="${currentFocus.projectId}"]`)?.focus();
      });
    } else if (currentFocus?.type === "session") {
      if (!expandedProjectSet.has(currentFocus.projectId)) {
        const next = new Set(expandedProjectSet);
        next.add(currentFocus.projectId);
        expandedProjects.set(next);
      }
      requestAnimationFrame(() => {
        sidebarEl?.querySelector<HTMLElement>(`[data-session-id="${currentFocus.sessionId}"]`)?.focus();
      });
    } else if (currentFocus?.type === "agent") {
      if (!expandedProjectSet.has(currentFocus.projectId)) {
        const next = new Set(expandedProjectSet);
        next.add(currentFocus.projectId);
        expandedProjects.set(next);
      }
      requestAnimationFrame(() => {
        sidebarEl?.querySelector<HTMLElement>(`[data-agent-id="${currentFocus.projectId}:${currentFocus.agentKind}"]`)?.focus();
      });
    } else if (currentFocus?.type === "agent-panel") {
      if (document.activeElement instanceof HTMLElement && sidebarEl.contains(document.activeElement)) {
        document.activeElement.blur();
      }
    }
  });

  $effect(() => {
    loadProjects();
  });

  $effect(() => {
    const unlisteners: (() => void)[] = [];

    for (const project of projectList) {
      unlisteners.push(listen<string>(`maintainer-status:${project.id}`, (payload) => {
        maintainerStatuses.update(m => {
          const next = new Map(m);
          next.set(project.id, payload as MaintainerStatus);
          return next;
        });
        if (payload !== "error") {
          maintainerErrors.update(m => {
            const next = new Map(m);
            next.delete(project.id);
            return next;
          });
        }
      }));

      unlisteners.push(listen<string>(`maintainer-error:${project.id}`, (payload) => {
        maintainerErrors.update(m => {
          const next = new Map(m);
          next.set(project.id, payload);
          return next;
        });
      }));

      unlisteners.push(listen<string>(`auto-worker-status:${project.id}`, (payload) => {
        try {
          const status = JSON.parse(payload);
          autoWorkerStatuses.update(m => {
            const next = new Map(m);
            next.set(project.id, status);
            return next;
          });
        } catch { /* ignore parse errors */ }
      }));
    }

    return () => {
      unlisteners.forEach(fn => fn());
    };
  });

  async function loadProjects() {
    try {
      const result = await refreshProjectsFromBackend();
      surfaceCorruptProjectWarnings(result.corrupt_entries);
    } catch (err) {
      showToast(String(err), "error");
    }
  }

  function surfaceCorruptProjectWarnings(entries: CorruptProjectEntry[]) {
    const unseen = entries.filter((entry) => !surfacedCorruptProjectWarnings.has(corruptProjectWarningKey(entry)));
    if (unseen.length === 0) return;

    const next = new Set(surfacedCorruptProjectWarnings);
    for (const entry of unseen) {
      next.add(corruptProjectWarningKey(entry));
    }
    surfacedCorruptProjectWarnings = next;

    if (unseen.length === 1) {
      const entry = unseen[0];
      showToast(`Detected corrupt project.json: ${entry.project_file} (${entry.error})`, "error");
      return;
    }

    showToast(
      `Detected ${unseen.length} corrupt project.json entries. Example: ${unseen[0].project_file}`,
      "error",
    );
  }

  function corruptProjectWarningKey(entry: CorruptProjectEntry) {
    return `${entry.project_file}:${entry.error}`;
  }

  function toggleProject(projectId: string) {
    const next = new Set(expandedProjectSet);
    if (next.has(projectId)) {
      next.delete(projectId);
    } else {
      next.add(projectId);
    }
    expandedProjects.set(next);
  }
</script>

<aside class="sidebar" bind:this={sidebarEl}>
  <div class="sidebar-header">
    <h2>{modeTitle}</h2>
  </div>

  <div class="project-list">
    {#if currentMode === "agents"}
      <AgentTree
        projects={projectList}
        {expandedProjectSet}
        {currentFocus}
        onToggleProject={toggleProject}
        onProjectFocus={(projectId) => {
          focusTarget.set({ type: "project", projectId });
        }}
        onAgentFocus={(agentKind, projectId) => {
          focusTarget.set({ type: "agent", agentKind, projectId });
        }}
      />
    {:else if currentMode === "kanban"}
      <ProjectList
        projects={projectList}
        {currentFocus}
        onProjectFocus={(projectId) => {
          focusTarget.set({ type: "project", projectId });
        }}
      />
    {:else}
      <ChatSessionList
        {currentFocus}
        onProjectFocus={(projectId) => {
          focusTarget.set({ type: "project", projectId });
        }}
        onNewChat={(projectId) => {
          const project = projectList.find((p) => p.id === projectId);
          if (!project) return;
          daemonStore.newChatTarget = { projectId, projectCwd: project.repo_path };
        }}
        onSelect={(sessionId) => {
          daemonStore.activeSessionId = sessionId;
        }}
      />
    {/if}
  </div>

  <div class="sidebar-footer">
    <button
      class="btn-help"
      class:active={showKeyHintsState.current}
      onclick={() => showKeyHints.update(v => !v)}
      title="Keyboard shortcuts (?)"
    >?</button>
  </div>
</aside>

<style>
  .sidebar {
    width: 250px;
    min-width: 250px;
    height: 100vh;
    background: var(--bg-surface);
    border-right: 1px solid var(--border-default);
    display: flex;
    flex-direction: column;
    color: var(--text-primary);
  }

  .sidebar-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border-default);
  }

  .sidebar-header h2 {
    font-size: 14px;
    font-weight: 600;
    margin: 0;
    flex: 1;
    text-align: center;
  }

  .project-list {
    flex: 1;
    overflow-y: auto;
  }

  .sidebar-footer {
    display: flex;
    justify-content: flex-end;
    border-top: 1px solid var(--border-default);
    padding: 0;
  }

  .btn-help {
    background: none;
    border: none;
    border-left: 1px solid var(--border-default);
    color: var(--text-secondary);
    width: 36px;
    padding: 8px 0;
    cursor: pointer;
    font-size: 13px;
    font-weight: 600;
    text-align: center;
    box-shadow: none;
    outline: none;
    flex-shrink: 0;
  }

  .btn-help:focus-visible {
    outline: 2px solid var(--focus-ring);
    outline-offset: -2px;
  }

  .btn-help:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  .btn-help.active {
    color: var(--text-emphasis);
  }
</style>
