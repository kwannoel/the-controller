<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { projects, activeSessionId, sessionStatuses, type Project } from "./stores";
  import { showToast } from "./toast";
  import FuzzyFinder from "./FuzzyFinder.svelte";
  import NewProjectModal from "./NewProjectModal.svelte";

  let showNewMenu = $state(false);
  let showFuzzyFinder = $state(false);
  let showNewProjectModal = $state(false);
  let expandedProjects = $state(new Set<string>());
  let showSessionMenu = $state<string | null>(null);

  let projectList: Project[] = $state([]);
  let activeSession: string | null = $state(null);
  let statuses: Map<string, "running" | "idle"> = $state(new Map());

  projects.subscribe((value) => {
    projectList = value;
  });

  activeSessionId.subscribe((value) => {
    activeSession = value;
  });

  sessionStatuses.subscribe((value) => {
    statuses = value;
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
    } catch (err) {
      showToast(String(err), "error");
    }
  }

  function toggleSessionMenu(projectId: string) {
    showSessionMenu = showSessionMenu === projectId ? null : projectId;
  }

  async function createRefinement(projectId: string) {
    const branchName = prompt("Enter branch name for refinement:");
    if (!branchName || !branchName.trim()) return;

    showSessionMenu = null;
    try {
      const sessionId: string = await invoke("create_refinement", {
        projectId,
        branchName: branchName.trim(),
      });
      sessionStatuses.update(m => {
        const next = new Map(m);
        next.set(sessionId, "running");
        return next;
      });
      activeSessionId.set(sessionId);
      await loadProjects();
      const next = new Set(expandedProjects);
      next.add(projectId);
      expandedProjects = next;
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
</script>

<aside class="sidebar">
  <div class="sidebar-header">
    <h2>Projects</h2>
    <div class="new-btn-wrapper">
      <button class="btn-new" onclick={() => showNewMenu = !showNewMenu}>+ New</button>
      {#if showNewMenu}
        <div class="new-menu">
          <button class="new-menu-item" onclick={() => { showNewMenu = false; showNewProjectModal = true; }}>Create New</button>
          <button class="new-menu-item" onclick={() => { showNewMenu = false; showFuzzyFinder = true; }}>Load Existing</button>
        </div>
      {/if}
    </div>
  </div>

  <div class="project-list">
    {#each projectList as project (project.id)}
      <div class="project-item">
        <div class="project-header">
          <button class="btn-expand" onclick={() => toggleProject(project.id)}>
            {expandedProjects.has(project.id) ? "\u25BC" : "\u25B6"}
          </button>
          <span class="project-name">{project.name}</span>
          <span class="session-count">{project.sessions.length}</span>
          <button
            class="btn-archive"
            onclick={(e: MouseEvent) => { e.stopPropagation(); archiveProject(project.id); }}
            title="Archive project"
          >Archive</button>
          <div class="add-session-wrapper">
            <button
              class="btn-add-session"
              onclick={(e: MouseEvent) => { e.stopPropagation(); toggleSessionMenu(project.id); }}
              title="New session"
            >+</button>
            {#if showSessionMenu === project.id}
              <div class="session-menu">
                <button
                  class="session-menu-item"
                  onclick={(e: MouseEvent) => { e.stopPropagation(); showSessionMenu = null; createSession(project.id); }}
                >Session</button>
                <button
                  class="session-menu-item"
                  onclick={(e: MouseEvent) => { e.stopPropagation(); createRefinement(project.id); }}
                >Refinement</button>
              </div>
            {/if}
          </div>
        </div>

        {#if expandedProjects.has(project.id)}
          <div class="session-list">
            {#each project.sessions as session (session.id)}
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
                <button
                  class="btn-close-session"
                  onclick={(e: MouseEvent) => { e.stopPropagation(); closeSession(project.id, session.id); }}
                  title="Close session"
                >&times;</button>
              </div>
            {/each}
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
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid #313244;
  }

  .sidebar-header h2 {
    font-size: 14px;
    font-weight: 600;
    margin: 0;
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

  .add-session-wrapper {
    position: relative;
  }

  .session-menu {
    position: absolute;
    top: 100%;
    right: 0;
    background: #1e1e2e;
    border: 1px solid #313244;
    border-radius: 4px;
    z-index: 10;
    min-width: 120px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  }

  .session-menu-item {
    display: block;
    width: 100%;
    padding: 6px 12px;
    background: none;
    border: none;
    color: #cdd6f4;
    font-size: 12px;
    text-align: left;
    cursor: pointer;
    box-shadow: none;
  }

  .session-menu-item:hover {
    background: #313244;
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
</style>
