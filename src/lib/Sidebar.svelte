<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { projects, activeSessionId, sessionStatuses, hotkeyAction, showKeyHints, jumpMode, generateJumpLabels, archiveView, focusedPanel, type Project, type JumpPhase } from "./stores";
  import { showToast } from "./toast";
  import FuzzyFinder from "./FuzzyFinder.svelte";
  import NewProjectModal from "./NewProjectModal.svelte";
  import DeleteProjectModal from "./DeleteProjectModal.svelte";

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
  let isArchiveView = $state(false);
  let archivedProjects: Project[] = $state([]);

  $effect(() => {
    const unsub = archiveView.subscribe((v) => { isArchiveView = v; });
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

  let isFocused = $state(false);
  $effect(() => {
    const unsub = focusedPanel.subscribe((v) => { isFocused = v === "sidebar"; });
    return unsub;
  });

  function handleSidebarFocusIn() {
    focusedPanel.set("sidebar");
  }

  let projectJumpLabels = $derived.by(() => {
    if (!jumpState || jumpState.phase !== 'project') return [];
    return generateJumpLabels(projectList.length);
  });

  let sessionJumpLabels = $derived.by(() => {
    const js = jumpState;
    if (!js || js.phase !== 'session') return [];
    const count = projectList.find(p => p.id === js.projectId)?.sessions.length ?? 0;
    return generateJumpLabels(count + 1);
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
        case "close-session": {
          if (activeSession) {
            const project = projectList.find((p) =>
              p.sessions.some((s) => s.id === activeSession),
            );
            if (project) closeSession(project.id, activeSession);
          }
          break;
        }
        case "focus-sidebar": {
          const firstSession = sidebarEl?.querySelector<HTMLElement>(".session-item");
          firstSession?.focus();
          break;
        }
        case "delete-project": {
          const project = projectList.find((p) =>
            p.sessions.some((s) => s.id === activeSession),
          ) ?? projectList[0];
          if (project) {
            deleteTarget = project;
          }
          break;
        }
        case "toggle-archive-view":
          archiveView.update((v) => !v);
          break;
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
      archivedProjects = await invoke<Project[]>("list_archived_projects");
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

  async function archiveProject(projectId: string) {
    if (!confirm("Archive this project? All sessions will be closed.")) return;
    try {
      await invoke("archive_project", { projectId });
      await loadProjects();
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  function getSessionStatus(sessionId: string): "running" | "idle" {
    return statuses.get(sessionId) ?? "idle";
  }

  function getGlobalSessionIndex(projectId: string, localIdx: number): number {
    let count = 0;
    for (const p of projectList) {
      if (p.id === projectId) return count + localIdx;
      count += p.sessions.length;
    }
    return count + localIdx;
  }
</script>

<aside class="sidebar" class:focused={isFocused} bind:this={sidebarEl} onfocusin={handleSidebarFocusIn}>
  {#if isArchiveView}
    <div class="sidebar-header">
      <button class="btn-back" onclick={() => archiveView.set(false)}>&larr;</button>
      <h2>Archived Projects</h2>
    </div>

    <div class="project-list">
      {#each archivedProjects as project (project.id)}
        <div class="project-item">
          <div class="project-header">
            <span class="project-name">{project.name}</span>
            <button
              class="btn-unarchive"
              onclick={() => unarchiveProject(project.id)}
            >Unarchive</button>
            <button
              class="btn-archive-delete"
              onclick={() => { deleteTarget = project; }}
            >Delete</button>
          </div>
        </div>
      {:else}
        <div class="empty-state">No archived projects</div>
      {/each}
    </div>
  {:else}
    <div class="sidebar-header">
      <h2>Projects</h2>
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
          <div class="project-header">
            <button class="btn-expand" onclick={() => toggleProject(project.id)}>
              {expandedProjects.has(project.id) ? "\u25BC" : "\u25B6"}
            </button>
            <span class="project-name">{project.name}</span>
            {#if jumpState?.phase === 'project' && projectJumpLabels[i]}
              <kbd class="jump-label">{projectJumpLabels[i]}</kbd>
            {:else if hintsVisible}
              <kbd class="hint">j</kbd>
            {/if}
            <span class="session-count">{project.sessions.length}</span>
            <button
              class="btn-archive"
              onclick={(e: MouseEvent) => { e.stopPropagation(); archiveProject(project.id); }}
              title="Archive project"
            >Archive</button>
            <button
              class="btn-add-session"
              onclick={(e: MouseEvent) => { e.stopPropagation(); createSession(project.id); }}
              title="New session"
            >+{#if hintsVisible}<kbd class="hint">c</kbd>{/if}</button>
          </div>

          {#if expandedProjects.has(project.id)}
            <div class="session-list">
              {#each project.sessions as session, sessionIdx (session.id)}
                {@const globalIdx = getGlobalSessionIndex(project.id, sessionIdx)}
                <div
                  class="session-item"
                  class:active={activeSession === session.id}
                  role="button"
                  tabindex="0"
                  onclick={() => selectSession(session.id)}
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
                  {:else}
                    {#if hintsVisible && globalIdx < 9}
                      <kbd class="hint">{globalIdx + 1}</kbd>
                    {/if}
                    {#if hintsVisible && activeSession === session.id}
                      <kbd class="hint">x</kbd>
                    {/if}
                  {/if}
                  <button
                    class="btn-close-session"
                    onclick={(e: MouseEvent) => { e.stopPropagation(); closeSession(project.id, session.id); }}
                    title="Close session"
                  >&times;</button>
                </div>
              {/each}
              {#if jumpState?.phase === 'session' && jumpState.projectId === project.id}
                <div class="session-item create-option">
                  <span class="status-dot">+</span>
                  <span class="session-label">New session</span>
                  <kbd class="jump-label">{sessionJumpLabels[project.sessions.length]}</kbd>
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
    border-right: 2px solid #313244;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    color: #cdd6f4;
    transition: border-color 0.15s ease;
  }

  .sidebar.focused {
    border-right-color: #89b4fa;
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
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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

  .session-item.create-option {
    color: #a6e3a1;
    font-style: italic;
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
