<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { projects, activeSessionId, sessionStatuses, hotkeyAction, showKeyHints, jumpMode, generateJumpLabels, archiveView, archivedProjects, focusTarget, expandedProjects, type Project, type JumpPhase, type FocusTarget, type SessionStatus } from "./stores";
  import { showToast } from "./toast";
  import { focusAfterSessionDelete, focusAfterProjectDelete } from "./focus-helpers";
  import FuzzyFinder from "./FuzzyFinder.svelte";
  import NewProjectModal from "./NewProjectModal.svelte";
  import DeleteProjectModal from "./DeleteProjectModal.svelte";
  import ConfirmModal from "./ConfirmModal.svelte";
  import DeleteSessionModal from "./DeleteSessionModal.svelte";

  let sidebarEl: HTMLElement | undefined = $state();
  let hintsVisible = $state(false);
  $effect(() => {
    const unsub = showKeyHints.subscribe((v) => { hintsVisible = v; });
    return unsub;
  });
  let showFuzzyFinder = $state(false);
  let showNewProjectModal = $state(false);
  let expandedProjectSet: Set<string> = $state(new Set());
  $effect(() => {
    const unsub = expandedProjects.subscribe((v) => { expandedProjectSet = v; });
    return unsub;
  });
  let deleteTarget: Project | null = $state(null);
  let deleteSessionTarget: { sessionId: string; projectId: string; label: string } | null = $state(null);
  let archiveSessionTarget: { sessionId: string; projectId: string; label: string } | null = $state(null);
  let archiveProjectTarget: Project | null = $state(null);
  let mergeSessionTarget: { sessionId: string; projectId: string; label: string } | null = $state(null);
  let mergeInProgress = $state(false);
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
  let projectList: Project[] = $state([]);
  let activeSession: string | null = $state(null);
  let statuses: Map<string, SessionStatus> = $state(new Map());
  const idleTimers = new Map<string, ReturnType<typeof setTimeout>>();
  const IDLE_TIMEOUT_MS = 3000;

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
      if (!expandedProjectSet.has(currentFocus.projectId)) {
        const next = new Set(expandedProjectSet);
        next.add(currentFocus.projectId);
        expandedProjects.set(next);
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
    if (jumpState?.phase === 'session' && !expandedProjectSet.has(jumpState.projectId)) {
      const next = new Set(expandedProjectSet);
      next.add(jumpState.projectId);
      expandedProjects.set(next);
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
            const searchList = isArchiveView ? archivedProjectList : projectList;
            const targetProjectId = action.projectId
              ?? searchList.find((p) => p.sessions.some((s) => s.id === targetSessionId))?.id;
            if (targetProjectId) {
              const project = searchList.find((p) => p.id === targetProjectId);
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
          const searchList = isArchiveView ? archivedProjectList : projectList;
          const project = action.projectId
            ? searchList.find((p) => p.id === action.projectId)
            : (searchList.find((p) =>
                p.sessions.some((s) => s.id === activeSession),
              ) ?? searchList[0]);
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
          if (project) archiveProjectTarget = project;
          break;
        }
        case "archive-session": {
          const proj = projectList.find((p) => p.id === action.projectId);
          const sess = proj?.sessions.find((s) => s.id === action.sessionId);
          if (sess) {
            archiveSessionTarget = {
              sessionId: action.sessionId,
              projectId: action.projectId,
              label: sess.label,
            };
          }
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
        case "merge-session": {
          const proj = projectList.find((p) => p.id === action.projectId);
          const sess = proj?.sessions.find((s) => s.id === action.sessionId);
          if (sess) {
            mergeSessionTarget = {
              sessionId: action.sessionId,
              projectId: action.projectId,
              label: sess.label,
            };
          }
          break;
        }
      }
    });
    return unsub;
  });

  $effect(() => {
    loadProjects();
  });

  function markSession(sessionId: string, status: SessionStatus) {
    sessionStatuses.update(m => {
      const next = new Map(m);
      next.set(sessionId, status);
      return next;
    });
  }

  function resetIdleTimer(sessionId: string) {
    const existing = idleTimers.get(sessionId);
    if (existing) clearTimeout(existing);
    idleTimers.set(sessionId, setTimeout(() => {
      // Only transition to idle if still working (not exited)
      sessionStatuses.update(m => {
        if (m.get(sessionId) === "working") {
          const next = new Map(m);
          next.set(sessionId, "idle");
          return next;
        }
        return m;
      });
    }, IDLE_TIMEOUT_MS));
  }

  $effect(() => {
    const unlisteners: (() => void)[] = [];
    let cancelled = false;

    for (const project of projectList) {
      for (const session of project.sessions) {
        listen<string>(`session-status-changed:${session.id}`, () => {
          const timer = idleTimers.get(session.id);
          if (timer) clearTimeout(timer);
          idleTimers.delete(session.id);
          markSession(session.id, "exited");
        }).then(unlisten => { if (!cancelled) unlisteners.push(unlisten); else unlisten(); });

        listen<string>(`pty-output:${session.id}`, () => {
          markSession(session.id, "working");
          resetIdleTimer(session.id);
        }).then(unlisten => { if (!cancelled) unlisteners.push(unlisten); else unlisten(); });
      }
    }

    return () => {
      cancelled = true;
      unlisteners.forEach(fn => fn());
      for (const timer of idleTimers.values()) {
        clearTimeout(timer);
      }
      idleTimers.clear();
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
    const next = new Set(expandedProjectSet);
    if (next.has(projectId)) {
      next.delete(projectId);
    } else {
      next.add(projectId);
    }
    expandedProjects.set(next);
  }

  async function createSession(projectId: string) {
    try {
      const sessionId: string = await invoke("create_session", {
        projectId,
      });
      markSession(sessionId, "working");
      activeSessionId.set(sessionId);
      await loadProjects();
      // Auto-expand the project
      const next = new Set(expandedProjectSet);
      next.add(projectId);
      expandedProjects.set(next);
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

  async function closeSession(projectId: string, sessionId: string, deleteWorktree: boolean) {
    try {
      const list = isArchiveView ? archivedProjectList : projectList;
      const nextFocus = focusAfterSessionDelete(list, projectId, sessionId, isArchiveView);

      await invoke("close_session", { projectId, sessionId, deleteWorktree });
      const timer = idleTimers.get(sessionId);
      if (timer) clearTimeout(timer);
      idleTimers.delete(sessionId);
      sessionStatuses.update(m => {
        const next = new Map(m);
        next.delete(sessionId);
        return next;
      });
      activeSessionId.update(current => {
        if (current !== sessionId) return current;
        if (nextFocus?.type === "session") return nextFocus.sessionId;
        return null;
      });
      focusTarget.set(nextFocus);
      await loadProjects();
      if (isArchiveView) await loadArchivedProjects();
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
      const timer = idleTimers.get(sessionId);
      if (timer) clearTimeout(timer);
      idleTimers.delete(sessionId);
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
      markSession(sessionId, "working");
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

  async function mergeSession(projectId: string, sessionId: string) {
    mergeInProgress = true;

    // Focus the terminal so user can watch Claude resolve conflicts if any
    activeSessionId.set(sessionId);
    setTimeout(() => {
      hotkeyAction.set({ type: "focus-terminal" });
      setTimeout(() => hotkeyAction.set(null), 0);
    }, 50);

    // Listen for intermediate merge status events
    let unlistenStatus: (() => void) | null = null;
    listen<string>("merge-status", (event) => {
      showToast(event.payload, "info");
    }).then(fn => { unlistenStatus = fn; });

    try {
      const result: { type: string; url?: string } = await invoke("merge_session_branch", { projectId, sessionId });
      if (result.type === "pr_created") {
        showToast(`PR created: ${result.url}`, "info");
      }
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      mergeInProgress = false;
      unlistenStatus?.();
    }
  }

  function getSessionStatus(sessionId: string): SessionStatus {
    return statuses.get(sessionId) ?? "idle";
  }

</script>

<aside class="sidebar" bind:this={sidebarEl}>
  <div class="sidebar-header">
    <h2>{isArchiveView ? "Archives" : "Projects"}</h2>
  </div>

  <div class="project-list">
    {#if isArchiveView}
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
              {expandedProjectSet.has(project.id) ? "\u25BC" : "\u25B6"}
            </button>
            <span class="project-name">{project.name}</span>
            {#if jumpState?.phase === 'project' && projectJumpLabels[i]}
              <kbd class="jump-label">{projectJumpLabels[i]}</kbd>
            {/if}
            <span class="session-count">{archivedSessions.length}</span>
          </div>

          {#if expandedProjectSet.has(project.id)}
            <div class="session-list">
              {#each archivedSessions as session, sessionIdx (session.id)}
                <div
                  class="session-item archived"
                  class:focus-target={currentFocus?.type === 'session' && currentFocus.sessionId === session.id}
                  data-session-id={session.id}
                  tabindex="0"
                  onfocusin={() => { focusTarget.set({ type: 'session', sessionId: session.id, projectId: project.id }); }}
                >
                  <span class="status-dot archived-dot">&cir;</span>
                  <span class="session-label">{session.label}</span>
                  {#if jumpState?.phase === 'session' && jumpState.projectId === project.id && sessionJumpLabels[sessionIdx]}
                    <kbd class="jump-label">{sessionJumpLabels[sessionIdx]}</kbd>
                  {/if}
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {:else}
        <div class="empty-state">No archived sessions</div>
      {/each}
    {:else}
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
              {expandedProjectSet.has(project.id) ? "\u25BC" : "\u25B6"}
            </button>
            <span class="project-name">{project.name}</span>
            {#if jumpState?.phase === 'project' && projectJumpLabels[i]}
              <kbd class="jump-label">{projectJumpLabels[i]}</kbd>
            {/if}
            <span class="session-count">{project.sessions.filter(s => !s.archived).length}</span>
          </div>

          {#if expandedProjectSet.has(project.id)}
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
                    class:idle={getSessionStatus(session.id) === "idle"}
                    class:working={getSessionStatus(session.id) === "working"}
                  >
                    {getSessionStatus(session.id) === "exited" ? "\u25CB" : "\u25CF"}
                  </span>
                  <span class="session-label">{session.label}</span>
                  {#if jumpState?.phase === 'session' && jumpState.projectId === project.id && sessionJumpLabels[sessionIdx]}
                    <kbd class="jump-label">{sessionJumpLabels[sessionIdx]}</kbd>
                  {/if}
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
    {/if}
  </div>

  <div class="sidebar-footer">
    <button
      class="footer-tab"
      class:active={!isArchiveView}
      onclick={() => archiveView.set(false)}
    >Active</button>
    <button
      class="footer-tab"
      class:active={isArchiveView}
      onclick={() => archiveView.set(true)}
    >Archives</button>
    <button
      class="btn-help"
      class:active={hintsVisible}
      onclick={() => showKeyHints.update(v => !v)}
      title="Keyboard shortcuts (?)"
    >?</button>
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

  {#if deleteSessionTarget}
    <DeleteSessionModal
      sessionLabel={deleteSessionTarget.label}
      isArchived={isArchiveView}
      onUntrack={() => {
        if (deleteSessionTarget) {
          closeSession(deleteSessionTarget.projectId, deleteSessionTarget.sessionId, false);
        }
        deleteSessionTarget = null;
      }}
      onDelete={() => {
        if (deleteSessionTarget) {
          closeSession(deleteSessionTarget.projectId, deleteSessionTarget.sessionId, true);
        }
        deleteSessionTarget = null;
      }}
      onClose={() => (deleteSessionTarget = null)}
    />
  {/if}

  {#if archiveSessionTarget}
    <ConfirmModal
      title="Archive Session"
      message={`Archive "${archiveSessionTarget.label}"? The terminal will be stopped.`}
      confirmLabel="Archive"
      onConfirm={() => {
        if (archiveSessionTarget) {
          archiveSession(archiveSessionTarget.projectId, archiveSessionTarget.sessionId);
        }
        archiveSessionTarget = null;
      }}
      onClose={() => (archiveSessionTarget = null)}
    />
  {/if}

  {#if archiveProjectTarget}
    <ConfirmModal
      title="Archive Project"
      message={`Archive "${archiveProjectTarget.name}" and all its sessions?`}
      confirmLabel="Archive"
      onConfirm={() => {
        if (archiveProjectTarget) {
          archiveProject(archiveProjectTarget.id);
        }
        archiveProjectTarget = null;
      }}
      onClose={() => (archiveProjectTarget = null)}
    />
  {/if}

  {#if mergeSessionTarget}
    <ConfirmModal
      title="Merge Session Branch"
      message={`Create PR to merge "${mergeSessionTarget.label}" into main?${mergeInProgress ? " Merging..." : ""}`}
      confirmLabel="Merge"
      onConfirm={() => {
        if (mergeSessionTarget && !mergeInProgress) {
          mergeSession(mergeSessionTarget.projectId, mergeSessionTarget.sessionId);
        }
        mergeSessionTarget = null;
      }}
      onClose={() => (mergeSessionTarget = null)}
    />
  {/if}

  {#if deleteTarget}
    <DeleteProjectModal
      projectId={deleteTarget.id}
      projectName={deleteTarget.name}
      onDeleted={async () => {
        const list = isArchiveView ? archivedProjectList : projectList;
        const nextFocus = focusAfterProjectDelete(list, deleteTarget!.id, expandedProjectSet, isArchiveView);
        activeSessionId.update(current => {
          if (deleteTarget!.sessions.some(s => s.id === current)) return nextFocus?.type === "session" ? nextFocus.sessionId : null;
          return current;
        });
        deleteTarget = null;
        await loadProjects();
        if (isArchiveView) await loadArchivedProjects();
        focusTarget.set(nextFocus);
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
    text-align: center;
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

  .empty-state {
    padding: 24px 16px;
    color: #6c7086;
    font-size: 13px;
    text-align: center;
  }

  /* Footer */
  .sidebar-footer {
    display: flex;
    align-items: center;
    border-top: 1px solid #313244;
    padding: 0;
  }

  .footer-tab {
    flex: 1;
    background: none;
    border: none;
    color: #6c7086;
    padding: 8px 0;
    font-size: 12px;
    cursor: pointer;
    box-shadow: none;
    text-align: center;
  }

  .footer-tab:hover {
    color: #cdd6f4;
    background: #313244;
  }

  .footer-tab.active {
    color: #cdd6f4;
    border-bottom: 2px solid #89b4fa;
  }

  .btn-help {
    background: none;
    border: none;
    border-left: 1px solid #313244;
    color: #6c7086;
    width: 36px;
    padding: 8px 0;
    cursor: pointer;
    font-size: 13px;
    font-weight: 600;
    text-align: center;
    box-shadow: none;
    flex-shrink: 0;
  }

  .btn-help:hover {
    color: #cdd6f4;
    background: #313244;
  }

  .btn-help.active {
    color: #89b4fa;
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
</style>
