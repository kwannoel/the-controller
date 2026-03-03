<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { projects, activeSessionId, sessionStatuses, hotkeyAction, showKeyHints, jumpMode, generateJumpLabels, archiveView, archivedProjects, focusTarget, type Project, type JumpPhase, type FocusTarget } from "./stores";
  import { showToast } from "./toast";
  import FuzzyFinder from "./FuzzyFinder.svelte";
  import NewProjectModal from "./NewProjectModal.svelte";
  import DeleteProjectModal from "./DeleteProjectModal.svelte";
  import ConfirmModal from "./ConfirmModal.svelte";

  let sidebarEl: HTMLElement | undefined = $state();
  let hintsVisible = $state(false);
  $effect(() => {
    const unsub = showKeyHints.subscribe((v) => { hintsVisible = v; });
    return unsub;
  });
  let showNewMenu = $state(false);
  let showFuzzyFinder = $state(false);
  let showNewProjectModal = $state(false);
  let expandedProjects = $state(new Set<string>());
  let deleteTarget: Project | null = $state(null);
  let deleteSessionTarget: { sessionId: string; projectId: string; label: string } | null = $state(null);
  let isArchiveView = $state(false);
  let archivedProjectList: Project[] = $state([]);

  $effect(() => {
    const unsub = archiveView.subscribe((v) => { isArchiveView = v; });
    return unsub;
  });

  $effect(() => {
    const unsub = archivedProjects.subscribe((v) => { archivedProjectList = v; });
    return unsub;
  });

  // Load archived projects when entering archive view
  $effect(() => {
    if (isArchiveView) {
      loadArchivedProjects();
    }
  });
  // Close dropdown menus on outside click
  $effect(() => {
    if (!showNewMenu) return;
    function handleClick() {
      showNewMenu = false;
    }
    const timer = setTimeout(() => window.addEventListener("click", handleClick), 0);
    return () => {
      clearTimeout(timer);
      window.removeEventListener("click", handleClick);
    };
  });

  let projectList: Project[] = $state([]);
  let activeSession: string | null = $state(null);
  let statuses: Map<string, "running" | "idle"> = $state(new Map());

  $effect(() => {
    const unsub = projects.subscribe((value) => { projectList = value; });
    return unsub;
  });

  $effect(() => {
    const unsub = activeSessionId.subscribe((value) => { activeSession = value; });
    return unsub;
  });

  $effect(() => {
    const unsub = sessionStatuses.subscribe((value) => { statuses = value; });
    return unsub;
  });

  let jumpState: JumpPhase = $state(null);
  $effect(() => {
    const unsub = jumpMode.subscribe((v) => { jumpState = v; });
    return unsub;
  });

  let currentFocus: FocusTarget = $state(null);
  $effect(() => {
    const unsub = focusTarget.subscribe((v) => { currentFocus = v; });
    return unsub;
  });

  // When focusTarget changes, expand and focus the relevant DOM element
  $effect(() => {
    if (currentFocus?.type === "session") {
      if (!expandedProjects.has(currentFocus.projectId)) {
        const next = new Set(expandedProjects);
        next.add(currentFocus.projectId);
        expandedProjects = next;
      }
      if (sidebarEl) {
        requestAnimationFrame(() => {
          const el = sidebarEl?.querySelector<HTMLElement>(`[data-session-id="${currentFocus.sessionId}"]`);
          if (el) el.focus();
        });
      }
    } else if (currentFocus?.type === "project") {
      if (sidebarEl) {
        requestAnimationFrame(() => {
          const el = sidebarEl?.querySelector<HTMLElement>(`[data-project-id="${currentFocus.projectId}"]`);
          if (el) el.focus();
        });
      }
    }
  });

  let projectJumpLabels = $derived.by(() => {
    if (!jumpState || jumpState.phase !== 'project') return [];
    const list = isArchiveView ? archivedProjectList : projectList;
    return generateJumpLabels(list.length);
  });

  let sessionJumpLabels = $derived.by(() => {
    const js = jumpState;
    if (!js || js.phase !== 'session') return [];
    const list = isArchiveView ? archivedProjectList : projectList;
    const project = list.find(p => p.id === js.projectId);
    if (!project) return [];
    const sessions = isArchiveView
      ? project.sessions.filter(s => s.archived)
      : project.sessions.filter(s => !s.archived);
    // In archive view, no "create new" option
    return generateJumpLabels(isArchiveView ? sessions.length : sessions.length + 1);
  });

  // Auto-expand project when entering session jump phase
  $effect(() => {
    if (jumpState?.phase === 'session' && !expandedProjects.has(jumpState.projectId)) {
      const next = new Set(expandedProjects);
      next.add(jumpState.projectId);
      expandedProjects = next;
    }
  });

  // React to hotkey actions
  $effect(() => {
    const unsub = hotkeyAction.subscribe((action) => {
      if (!action) return;
      switch (action.type) {
        case "open-fuzzy-finder":
          showFuzzyFinder = true;
          break;
        case "open-new-project":
          showNewProjectModal = true;
          break;
        case "create-session": {
          const project = action.projectId
            ? projectList.find((p) => p.id === action.projectId)
            : (projectList.find((p) =>
                p.sessions.some((s) => s.id === activeSession),
              ) ?? projectList[0]);
          if (project) createSession(project.id);
          break;
        }
        case "delete-session": {
          const targetSessionId = action.sessionId ?? activeSession;
          if (targetSessionId) {
            const targetProjectId = action.projectId
              ?? projectList.find((p) => p.sessions.some((s) => s.id === targetSessionId))?.id;
            if (targetProjectId) {
              const project = projectList.find((p) => p.id === targetProjectId);
              const session = project?.sessions.find((s) => s.id === targetSessionId);
              deleteSessionTarget = {
                sessionId: targetSessionId,
                projectId: targetProjectId,
                label: session?.label ?? "this session",
              };
            }
          }
          break;
        }
        case "delete-project": {
          const project = action.projectId
            ? projectList.find((p) => p.id === action.projectId)
            : (projectList.find((p) =>
                p.sessions.some((s) => s.id === activeSession),
              ) ?? projectList[0]);
          if (project) {
            deleteTarget = project;
          }
          break;
        }
        case "archive-project": {
          const project = action.projectId
            ? projectList.find((p) => p.id === action.projectId)
            : (projectList.find((p) =>
                p.sessions.some((s) => s.id === activeSession),
              ) ?? projectList[0]);
          if (project) archiveProject(project.id);
          break;
        }
        case "archive-session": {
          archiveSession(action.projectId, action.sessionId);
          break;
        }
        case "unarchive-session": {
          unarchiveSession(action.projectId, action.sessionId);
          break;
        }
        case "unarchive-project": {
          unarchiveProject(action.projectId);
          break;
        }
        case "toggle-archive-view": {
          archiveView.update(v => !v);
          break;
        }
      }
    });
    return unsub;
  });

  $effect(() => {
    loadProjects();
  });

  $effect(() => {
    const unlisteners: (() => void)[] = [];

    for (const project of projectList) {
      for (const session of project.sessions) {
        listen<string>(`session-status-changed:${session.id}`, () => {
          sessionStatuses.update(m => {
            const next = new Map(m);
            next.set(session.id, "idle");
            return next;
          });
        }).then(unlisten => unlisteners.push(unlisten));
      }
    }

    return () => {
      unlisteners.forEach(fn => fn());
    };
  });

  async function loadProjects() {
    try {
      const result: Project[] = await invoke("list_projects");
      projects.set(result);
    } catch (err) {
      showToast(String(err), "error");
    }
  }

  async function loadArchivedProjects() {
    try {
      const result = await invoke<Project[]>("list_archived_projects");
      archivedProjects.set(result);
    } catch (err) {
      showToast(String(err), "error");
    }
  }

  async function unarchiveProject(projectId: string) {
    try {
      await invoke("unarchive_project", { projectId });
      await loadArchivedProjects();
      await loadProjects();
    } catch (err) {
      showToast(String(err), "error");
    }
  }

  function toggleProject(projectId: string) {
    const next = new Set(expandedProjects);
    if (next.has(projectId)) {
      next.delete(projectId);
    } else {
      next.add(projectId);
    }
    expandedProjects = next;
  }

  async function createSession(projectId: string) {
    try {
      const sessionId: string = await invoke("create_session", {
        projectId,
      });
      sessionStatuses.update(m => {
        const next = new Map(m);
        next.set(sessionId, "running");
        return next;
      });
      activeSessionId.set(sessionId);
      await loadProjects();
      // Auto-expand the project
      const next = new Set(expandedProjects);
      next.add(projectId);
      expandedProjects = next;
      // Auto-focus the terminal (slight delay for component mount)
      setTimeout(() => {
        hotkeyAction.set({ type: "focus-terminal" });
        setTimeout(() => hotkeyAction.set(null), 0);
      }, 50);
    } catch (err) {
      showToast(String(err), "error");
    }
  }

  function selectSession(sessionId: string) {
    activeSessionId.set(sessionId);
  }

  async function closeSession(projectId: string, sessionId: string) {
    try {
      await invoke("close_session", { projectId, sessionId });
      // Remove from status tracking
      sessionStatuses.update(m => {
        const next = new Map(m);
        next.delete(sessionId);
        return next;
      });
      // Clear active session if it was the closed one
      activeSessionId.update(current => current === sessionId ? null : current);
      // Reload projects
      await loadProjects();
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function archiveSession(projectId: string, sessionId: string) {
    try {
      // Find the active session above the one being archived
      const project = projectList.find(p => p.id === projectId);
      const activeSessions = project?.sessions.filter(s => !s.archived) ?? [];
      const idx = activeSessions.findIndex(s => s.id === sessionId);
      const prevSession = idx > 0 ? activeSessions[idx - 1] : null;

      await invoke("archive_session", { projectId, sessionId });
      sessionStatuses.update(m => {
        const next = new Map(m);
        next.delete(sessionId);
        return next;
      });
      activeSessionId.update(current => current === sessionId ? (prevSession?.id ?? null) : current);
      if (prevSession) {
        focusTarget.set({ type: "session", sessionId: prevSession.id, projectId });
      } else {
        focusTarget.set({ type: "project", projectId });
      }
      await loadProjects();
      if (isArchiveView) await loadArchivedProjects();
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function unarchiveSession(projectId: string, sessionId: string) {
    try {
      await invoke("unarchive_session", { projectId, sessionId });
      sessionStatuses.update(m => {
        const next = new Map(m);
        next.set(sessionId, "running");
        return next;
      });
      activeSessionId.set(sessionId);
      await loadProjects();
      if (isArchiveView) await loadArchivedProjects();
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function archiveProject(projectId: string) {
    try {
      await invoke("archive_project", { projectId });
      activeSessionId.update(current => {
        const project = projectList.find(p => p.id === projectId);
        if (project?.sessions.some(s => s.id === current)) return null;
        return current;
      });
      await loadProjects();
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  function getSessionStatus(sessionId: string): "running" | "idle" {
    return statuses.get(sessionId) ?? "idle";
  }

</script>

<aside class="sidebar" bind:this={sidebarEl}>
  {#if isArchiveView}
    <div class="sidebar-header">
      <button class="btn-back" onclick={() => archiveView.set(false)}>&larr;</button>
      <h2>Archived Projects</h2>
    </div>

    <div class="project-list">
      {#each archivedProjectList as project, i (project.id)}
        {@const archivedSessions = project.sessions.filter(s => s.archived)}
        <div class="project-item">
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="project-header"
            class:focus-target={currentFocus?.type === 'project' && currentFocus.projectId === project.id}
            tabindex="0"
            data-project-id={project.id}
            onfocusin={(e: FocusEvent) => { if (e.target === e.currentTarget) focusTarget.set({ type: 'project', projectId: project.id }); }}
          >
            <button class="btn-expand" onclick={() => toggleProject(project.id)}>
              {expandedProjects.has(project.id) ? "\u25BC" : "\u25B6"}
            </button>
            <span class="project-name">{project.name}</span>
            {#if jumpState?.phase === 'project' && projectJumpLabels[i]}
              <kbd class="jump-label">{projectJumpLabels[i]}</kbd>
            {/if}
            <span class="session-count">{archivedSessions.length}</span>
            <button
              class="btn-unarchive"
              onclick={(e: MouseEvent) => { e.stopPropagation(); unarchiveProject(project.id); }}
              title="Restore all sessions"
            >Restore All</button>
            <button
              class="btn-archive-delete"
              onclick={(e: MouseEvent) => { e.stopPropagation(); deleteTarget = project; }}
              title="Delete project"
            >Delete</button>
          </div>

          {#if expandedProjects.has(project.id)}
            <div class="session-list">
              {#each archivedSessions as session, sessionIdx (session.id)}
                <div class="session-item archived">
                  <span class="status-dot archived-dot">&cir;</span>
                  <span class="session-label">{session.label}</span>
                  {#if jumpState?.phase === 'session' && jumpState.projectId === project.id && sessionJumpLabels[sessionIdx]}
                    <kbd class="jump-label">{sessionJumpLabels[sessionIdx]}</kbd>
                  {/if}
                  <button
                    class="btn-unarchive-session"
                    onclick={(e: MouseEvent) => { e.stopPropagation(); unarchiveSession(project.id, session.id); }}
                    title="Restore session"
                  >Restore</button>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {:else}
        <div class="empty-state">No archived sessions</div>
      {/each}
    </div>
  {:else}
    <div class="sidebar-header">
      <h2>Projects</h2>
      <button
        class="btn-archives"
        onclick={() => archiveView.set(true)}
        title="View archives (Shift+A)"
      >Archives</button>
      <button
        class="btn-hint-toggle"
        class:active={hintsVisible}
        onclick={() => showKeyHints.update(v => !v)}
        title="Toggle key hints (?)"
      >?</button>
      <div class="new-btn-wrapper">
        <button class="btn-new" onclick={() => showNewMenu = !showNewMenu}>+ New{#if hintsVisible}<kbd class="hint">n</kbd>{/if}</button>
        {#if showNewMenu}
          <div class="new-menu">
            <button class="new-menu-item" onclick={() => { showNewMenu = false; showNewProjectModal = true; }}>Create New</button>
            <button class="new-menu-item" onclick={() => { showNewMenu = false; showFuzzyFinder = true; }}>Load Existing{#if hintsVisible}<kbd class="hint">f</kbd>{/if}</button>
          </div>
        {/if}
      </div>
    </div>

    <div class="project-list">
      {#each projectList as project, i (project.id)}
        <div class="project-item">
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="project-header"
            class:focus-target={currentFocus?.type === 'project' && currentFocus.projectId === project.id}
            tabindex="0"
            data-project-id={project.id}
            onfocusin={(e: FocusEvent) => { if (e.target === e.currentTarget) focusTarget.set({ type: 'project', projectId: project.id }); }}
          >
            <button class="btn-expand" onclick={() => toggleProject(project.id)}>
              {expandedProjects.has(project.id) ? "\u25BC" : "\u25B6"}
            </button>
            <span class="project-name">{project.name}</span>
            {#if jumpState?.phase === 'project' && projectJumpLabels[i]}
              <kbd class="jump-label">{projectJumpLabels[i]}</kbd>
            {:else if hintsVisible}
              <kbd class="hint">j</kbd>
            {/if}
            <span class="session-count">{project.sessions.filter(s => !s.archived).length}</span>
            <button
              class="btn-archive"
              onclick={(e: MouseEvent) => { e.stopPropagation(); archiveProject(project.id); }}
              title="Archive project"
            >Archive</button>
            <button
              class="btn-add-session"
              onclick={(e: MouseEvent) => { e.stopPropagation(); createSession(project.id); }}
              title="New session"
            >+</button>
          </div>

          {#if expandedProjects.has(project.id)}
            {@const activeSessions = project.sessions.filter(s => !s.archived)}
            <div class="session-list">
              {#each activeSessions as session, sessionIdx (session.id)}
                <div
                  class="session-item"
                  class:active={activeSession === session.id}
                  class:focus-target={currentFocus?.type === 'session' && currentFocus.sessionId === session.id}
                  data-session-id={session.id}
                  role="button"
                  tabindex="0"
                  onclick={() => { selectSession(session.id); focusTarget.set({ type: 'session', sessionId: session.id, projectId: project.id }); }}
                  onfocusin={() => { focusTarget.set({ type: 'session', sessionId: session.id, projectId: project.id }); }}
                  onkeydown={(e: KeyboardEvent) => { if (e.key === 'Enter' || e.key === ' ') selectSession(session.id); }}
                >
                  <span
                    class="status-dot"
                    class:running={getSessionStatus(session.id) === "running"}
                  >
                    {getSessionStatus(session.id) === "running" ? "\u25CF" : "\u25CB"}
                  </span>
                  <span class="session-label">{session.label}</span>
                  {#if jumpState?.phase === 'session' && jumpState.projectId === project.id && sessionJumpLabels[sessionIdx]}
                    <kbd class="jump-label">{sessionJumpLabels[sessionIdx]}</kbd>
                  {/if}
                  <button
                    class="btn-close-session"
                    onclick={(e: MouseEvent) => { e.stopPropagation(); deleteSessionTarget = { sessionId: session.id, projectId: project.id, label: session.label }; }}
                    title="Delete session"
                  >&times;</button>
                </div>
              {/each}
              {#if jumpState?.phase === 'session' && jumpState.projectId === project.id}
                <div class="session-item create-option">
                  <span class="status-dot">+</span>
                  <span class="session-label">New session</span>
                  <kbd class="jump-label">{sessionJumpLabels[activeSessions.length]}</kbd>
                </div>
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    </div>

    {#if showFuzzyFinder}
      <FuzzyFinder
        onSelect={async (entry) => {
          showFuzzyFinder = false;
          try {
            await invoke("load_project", { name: entry.name, repoPath: entry.path });
            await loadProjects();
          } catch (e) {
            showToast(String(e), "error");
          }
        }}
        onClose={() => (showFuzzyFinder = false)}
      />
    {/if}

    {#if showNewProjectModal}
      <NewProjectModal
        onCreated={async () => {
          showNewProjectModal = false;
          await loadProjects();
        }}
        onClose={() => (showNewProjectModal = false)}
      />
    {/if}
  {/if}

  {#if deleteSessionTarget}
    <ConfirmModal
      title="Delete Session"
      message={`Delete "${deleteSessionTarget.label}"? The terminal process will be terminated.`}
      confirmLabel="Delete"
      onConfirm={() => {
        if (deleteSessionTarget) {
          closeSession(deleteSessionTarget.projectId, deleteSessionTarget.sessionId);
        }
        deleteSessionTarget = null;
      }}
      onClose={() => (deleteSessionTarget = null)}
    />
  {/if}

  {#if deleteTarget}
    <DeleteProjectModal
      projectId={deleteTarget.id}
      projectName={deleteTarget.name}
      onDeleted={async () => {
        deleteTarget = null;
        await loadProjects();
        if (isArchiveView) await loadArchivedProjects();
      }}
      onClose={() => (deleteTarget = null)}
    />
  {/if}
</aside>

<style>
  .sidebar {
    width: 250px;
    min-width: 250px;
    height: 100vh;
    background: #1e1e2e;
    border-right: 1px solid #313244;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    color: #cdd6f4;
  }

  .sidebar-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 16px;
    border-bottom: 1px solid #313244;
  }

  .sidebar-header h2 {
    font-size: 14px;
    font-weight: 600;
    margin: 0;
    flex: 1;
  }

  .new-btn-wrapper {
    position: relative;
  }

  .btn-new {
    background: none;
    border: 1px solid #313244;
    color: #cdd6f4;
    padding: 4px 10px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 12px;
    box-shadow: none;
  }

  .btn-new:hover {
    background: #313244;
  }

  .new-menu {
    position: absolute;
    top: 100%;
    right: 0;
    background: #1e1e2e;
    border: 1px solid #313244;
    border-radius: 4px;
    z-index: 10;
    min-width: 140px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  }

  .new-menu-item {
    display: block;
    width: 100%;
    padding: 8px 12px;
    background: none;
    border: none;
    color: #cdd6f4;
    font-size: 12px;
    text-align: left;
    cursor: pointer;
    box-shadow: none;
  }

  .new-menu-item:hover {
    background: #313244;
  }

  .project-list {
    flex: 1;
    overflow-y: auto;
  }

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

  .btn-add-session {
    background: none;
    border: none;
    color: #6c7086;
    cursor: pointer;
    padding: 0 4px;
    font-size: 16px;
    line-height: 1;
    box-shadow: none;
  }

  .btn-add-session:hover {
    color: #cdd6f4;
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

  .session-item.create-option {
    color: #a6e3a1;
    font-style: italic;
  }

  .session-item.archived {
    opacity: 0.5;
    cursor: default;
  }

  .archived-dot {
    color: #6c7086;
  }

  .btn-unarchive-session {
    background: none;
    border: none;
    color: #6c7086;
    cursor: pointer;
    padding: 2px 6px;
    font-size: 11px;
    box-shadow: none;
    opacity: 0;
    margin-left: auto;
    border-radius: 4px;
  }

  .session-item.archived:hover .btn-unarchive-session {
    opacity: 1;
  }

  .btn-unarchive-session:hover {
    color: #a6e3a1;
    background: #45475a;
  }

  .status-dot {
    font-size: 10px;
    color: #6c7086;
  }

  .status-dot.running {
    color: #a6e3a1;
  }

  .session-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
  }

  .btn-close-session {
    background: none;
    border: none;
    color: #6c7086;
    cursor: pointer;
    padding: 0 4px;
    font-size: 14px;
    line-height: 1;
    box-shadow: none;
    opacity: 0;
    margin-left: auto;
  }

  .session-item:hover .btn-close-session {
    opacity: 1;
  }

  .btn-close-session:hover {
    color: #f38ba8;
  }

  .btn-archive {
    background: none;
    border: none;
    color: #6c7086;
    cursor: pointer;
    padding: 2px 6px;
    font-size: 11px;
    box-shadow: none;
    opacity: 0;
    border-radius: 4px;
  }

  .project-header:hover .btn-archive {
    opacity: 1;
  }

  .btn-archive:hover {
    color: #cdd6f4;
    background: #45475a;
  }

  .btn-archives {
    background: none;
    border: 1px solid #313244;
    color: #6c7086;
    padding: 4px 8px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 11px;
    box-shadow: none;
    flex-shrink: 0;
  }

  .btn-archives:hover {
    color: #cdd6f4;
    background: #313244;
  }

  .btn-hint-toggle {
    background: none;
    border: 1px solid #313244;
    color: #6c7086;
    width: 24px;
    height: 24px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 13px;
    font-weight: 600;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    box-shadow: none;
    flex-shrink: 0;
  }

  .btn-hint-toggle:hover {
    color: #cdd6f4;
    background: #313244;
  }

  .btn-hint-toggle.active {
    color: #89b4fa;
    border-color: #89b4fa;
  }

  .hint {
    background: #313244;
    color: #89b4fa;
    padding: 0 5px;
    border-radius: 3px;
    font-family: monospace;
    font-size: 10px;
    font-weight: 600;
    line-height: 16px;
    white-space: nowrap;
    flex-shrink: 0;
    margin-left: 4px;
  }

  .hint-subtle {
    color: #6c7086;
    background: rgba(49, 50, 68, 0.6);
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

  .btn-back {
    background: none;
    border: none;
    color: #6c7086;
    cursor: pointer;
    padding: 0 4px;
    font-size: 16px;
    box-shadow: none;
  }

  .btn-back:hover {
    color: #cdd6f4;
  }

  .btn-unarchive {
    background: none;
    border: none;
    color: #6c7086;
    cursor: pointer;
    padding: 2px 6px;
    font-size: 11px;
    box-shadow: none;
    opacity: 0;
    border-radius: 4px;
  }

  .project-header:hover .btn-unarchive {
    opacity: 1;
  }

  .btn-unarchive:hover {
    color: #a6e3a1;
    background: #45475a;
  }

  .btn-archive-delete {
    background: none;
    border: none;
    color: #6c7086;
    cursor: pointer;
    padding: 2px 6px;
    font-size: 11px;
    box-shadow: none;
    opacity: 0;
    border-radius: 4px;
  }

  .project-header:hover .btn-archive-delete {
    opacity: 1;
  }

  .btn-archive-delete:hover {
    color: #f38ba8;
    background: #45475a;
  }

  .empty-state {
    padding: 24px 16px;
    color: #6c7086;
    font-size: 13px;
    text-align: center;
  }
</style>
