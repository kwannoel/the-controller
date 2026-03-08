<script lang="ts">
  import { fromStore } from "svelte/store";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { projects, activeSessionId, sessionStatuses, maintainerStatuses, autoWorkerStatuses, hotkeyAction, showKeyHints, jumpMode, generateJumpLabels, archiveView, archivedProjects, focusTarget, expandedProjects, focusTerminalSoon, workspaceMode, type Project, type JumpPhase, type FocusTarget, type SessionStatus, type AutoWorkerStatus } from "./stores";
  import { showToast } from "./toast";
  import { focusAfterSessionDelete, focusAfterProjectDelete } from "./focus-helpers";
  import FuzzyFinder from "./FuzzyFinder.svelte";
  import NewProjectModal from "./NewProjectModal.svelte";
  import DeleteProjectModal from "./DeleteProjectModal.svelte";
  import ConfirmModal from "./ConfirmModal.svelte";
  import DeleteSessionModal from "./DeleteSessionModal.svelte";
  import ProjectTree from "./sidebar/ProjectTree.svelte";
  import AgentTree from "./sidebar/AgentTree.svelte";

  let sidebarEl: HTMLElement | undefined = $state();
  const showKeyHintsState = fromStore(showKeyHints);
  let showFuzzyFinder = $state(false);
  let showNewProjectModal = $state(false);
  const expandedProjectsState = fromStore(expandedProjects);
  let expandedProjectSet: Set<string> = $derived(expandedProjectsState.current);
  let deleteTarget: Project | null = $state(null);
  let deleteSessionTarget: { sessionId: string; projectId: string; label: string } | null = $state(null);
  let archiveSessionTarget: { sessionId: string; projectId: string; label: string } | null = $state(null);
  let archiveProjectTarget: Project | null = $state(null);
  let mergeSessionTarget: { sessionId: string; projectId: string; label: string } | null = $state(null);
  let mergeInProgress = $state(false);
  let finishBranchTarget: { sessionId: string; kind?: string } | null = $state(null);
  const archiveViewState = fromStore(archiveView);
  let isArchiveView = $derived(archiveViewState.current);
  const workspaceModeState = fromStore(workspaceMode);
  let currentMode = $derived(workspaceModeState.current);
  const archivedProjectsState = fromStore(archivedProjects);
  let archivedProjectList: Project[] = $derived(archivedProjectsState.current);

  // Load archived projects when entering archive view
  $effect(() => {
    if (isArchiveView) {
      loadArchivedProjects();
    }
  });
  const projectsState = fromStore(projects);
  let projectList: Project[] = $derived(projectsState.current);
  const activeSessionIdState = fromStore(activeSessionId);
  let activeSession: string | null = $derived(activeSessionIdState.current);
  const sessionStatusesState = fromStore(sessionStatuses);
  let statuses: Map<string, SessionStatus> = $derived(sessionStatusesState.current);
  const idleTimers = new Map<string, ReturnType<typeof setTimeout>>();
  const IDLE_DEBOUNCE_MS = 1500;

  const jumpModeState = fromStore(jumpMode);
  let jumpState: JumpPhase = $derived(jumpModeState.current);

  const focusTargetState = fromStore(focusTarget);
  let currentFocus: FocusTarget = $derived(focusTargetState.current);

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
          if (project) createSession(project.id, action.kind);
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
        case "finish-branch": {
          finishBranchTarget = { sessionId: action.sessionId, kind: action.kind };
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

  $effect(() => {
    const unlisteners: (() => void)[] = [];
    let cancelled = false;

    for (const project of projectList) {
      for (const session of project.sessions) {
        listen<string>(`session-status-changed:${session.id}`, () => {
          markSession(session.id, "exited");
        }).then(unlisten => { if (!cancelled) unlisteners.push(unlisten); else unlisten(); });

        // Cleanup: backend already deleted the session and worktree, just refresh.
        listen<string>(`session-cleanup:${session.id}`, () => {
          const nextFocus = focusAfterSessionDelete(projectList, project.id, session.id, isArchiveView);
          clearSessionTracking(session.id);
          activeSessionId.update(current => {
            if (current !== session.id) return current;
            if (nextFocus?.type === "session") return nextFocus.sessionId;
            return null;
          });
          focusTarget.set(nextFocus);
          refreshProjectLists();
        }).then(unlisten => { if (!cancelled) unlisteners.push(unlisten); else unlisten(); });

        // Hook-based status: precise idle/working from Claude Code hooks.
        // Debounce idle transitions to avoid flickering between tool calls
        // (Stop hook fires after each assistant turn, even mid-task).
        listen<string>(`session-status-hook:${session.id}`, (event) => {
          const status = event.payload as SessionStatus;
          if (status === "working") {
            const pending = idleTimers.get(session.id);
            if (pending) { clearTimeout(pending); idleTimers.delete(session.id); }
            markSession(session.id, "working");
          } else if (status === "idle") {
            const pending = idleTimers.get(session.id);
            if (pending) clearTimeout(pending);
            idleTimers.set(session.id, setTimeout(() => {
              idleTimers.delete(session.id);
              markSession(session.id, "idle");
            }, IDLE_DEBOUNCE_MS));
          }
        }).then(unlisten => { if (!cancelled) unlisteners.push(unlisten); else unlisten(); });
      }

      listen<string>(`maintainer-status:${project.id}`, (event) => {
        maintainerStatuses.update(m => {
          const next = new Map(m);
          next.set(project.id, event.payload as MaintainerStatus);
          return next;
        });
      }).then(unlisten => { if (!cancelled) unlisteners.push(unlisten); else unlisten(); });

      listen<string>(`auto-worker-status:${project.id}`, (event) => {
        try {
          const status = JSON.parse(event.payload);
          autoWorkerStatuses.update(m => {
            const next = new Map(m);
            next.set(project.id, status);
            return next;
          });
        } catch { /* ignore parse errors */ }
      }).then(unlisten => { if (!cancelled) unlisteners.push(unlisten); else unlisten(); });
    }

    return () => {
      cancelled = true;
      unlisteners.forEach(fn => fn());
      for (const timer of idleTimers.values()) clearTimeout(timer);
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

  async function createSession(projectId: string, kind?: string) {
    try {
      const sessionId: string = await invoke("create_session", {
        projectId,
        kind: kind ?? "claude",
      });
      markSession(sessionId, "working");
      activeSessionId.set(sessionId);
      await loadProjects();
      // Auto-expand the project
      const next = new Set(expandedProjectSet);
      next.add(projectId);
      expandedProjects.set(next);
      // Auto-focus the terminal (slight delay for component mount)
      focusTerminalSoon();
    } catch (err) {
      showToast(String(err), "error");
    }
  }

  function selectSession(sessionId: string) {
    activeSessionId.set(sessionId);
  }

  async function maybeRemoveInProgressLabel(projectId: string, sessionId: string, fromArchivedList = false) {
    const list = fromArchivedList ? archivedProjectList : projectList;
    const project = list.find((p) => p.id === projectId);
    const session = project?.sessions.find((s) => s.id === sessionId);
    if (session?.github_issue && project) {
      invoke("remove_github_label", {
        repoPath: project.repo_path,
        issueNumber: session.github_issue.number,
        label: "in-progress",
      }).catch(() => {});
    }
  }

  function clearSessionTracking(sessionId: string) {
    const timer = idleTimers.get(sessionId);
    if (timer) {
      clearTimeout(timer);
      idleTimers.delete(sessionId);
    }
    sessionStatuses.update((m) => {
      const next = new Map(m);
      next.delete(sessionId);
      return next;
    });
  }

  async function refreshProjectLists() {
    await loadProjects();
    if (isArchiveView) await loadArchivedProjects();
  }

  async function closeSession(projectId: string, sessionId: string, deleteWorktree: boolean) {
    try {
      const list = isArchiveView ? archivedProjectList : projectList;
      const nextFocus = focusAfterSessionDelete(list, projectId, sessionId, isArchiveView);

      await maybeRemoveInProgressLabel(projectId, sessionId, isArchiveView);

      await invoke("close_session", { projectId, sessionId, deleteWorktree });
      clearSessionTracking(sessionId);
      activeSessionId.update(current => {
        if (current !== sessionId) return current;
        if (nextFocus?.type === "session") return nextFocus.sessionId;
        return null;
      });
      focusTarget.set(nextFocus);
      await refreshProjectLists();
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

      await maybeRemoveInProgressLabel(projectId, sessionId);

      await invoke("archive_session", { projectId, sessionId });
      clearSessionTracking(sessionId);
      activeSessionId.update(current => current === sessionId ? (prevSession?.id ?? null) : current);
      if (prevSession) {
        focusTarget.set({ type: "session", sessionId: prevSession.id, projectId });
      } else {
        focusTarget.set({ type: "project", projectId });
      }
      await refreshProjectLists();
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function unarchiveSession(projectId: string, sessionId: string) {
    try {
      await invoke("unarchive_session", { projectId, sessionId });
      markSession(sessionId, "working");
      activeSessionId.set(sessionId);
      await refreshProjectLists();
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
    focusTerminalSoon();

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
    <h2>{isArchiveView ? "Archives" : currentMode === "agents" ? "Agents" : "Projects"}</h2>
  </div>

  <div class="project-list">
    {#if currentMode === "agents"}
      <AgentTree
        projects={projectList}
        {currentFocus}
        onProjectFocus={(projectId) => {
          focusTarget.set({ type: "project", projectId });
        }}
      />
    {:else}
      <ProjectTree
        projects={isArchiveView ? archivedProjectList : projectList}
        mode={isArchiveView ? "archived" : "active"}
        {expandedProjectSet}
        {activeSession}
        {currentFocus}
        jumpState={jumpState}
        {projectJumpLabels}
        {getSessionStatus}
        onToggleProject={toggleProject}
        onProjectFocus={(projectId) => {
          focusTarget.set({ type: "project", projectId });
        }}
        onSessionFocus={(sessionId, projectId) => {
          focusTarget.set({ type: "session", sessionId, projectId });
        }}
        onSessionSelect={(sessionId, projectId) => {
          selectSession(sessionId);
          focusTarget.set({ type: "session", sessionId, projectId });
        }}
      />
    {/if}
  </div>

  <div class="sidebar-footer">
    {#if currentMode !== "agents"}
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
    {:else}
      <div class="footer-spacer"></div>
    {/if}
    <button
      class="btn-help"
      class:active={showKeyHintsState.current}
      onclick={() => showKeyHints.update(v => !v)}
      title="Keyboard shortcuts (?)"
    >?</button>
  </div>

  {#if showFuzzyFinder}
    <FuzzyFinder
      onSelect={async (entry) => {
        showFuzzyFinder = false;
        try {
          const project = await invoke<Project>("load_project", { name: entry.name, repoPath: entry.path });
          await loadProjects();
          expandedProjects.update(s => { const next = new Set(s); next.add(project.id); return next; });
          focusTarget.set({ type: "project", projectId: project.id });
        } catch (e) {
          showToast(String(e), "error");
        }
      }}
      onClose={() => (showFuzzyFinder = false)}
    />
  {/if}

  {#if showNewProjectModal}
    <NewProjectModal
      onCreated={async (project) => {
        showNewProjectModal = false;
        await loadProjects();
        expandedProjects.update(s => { const next = new Set(s); next.add(project.id); return next; });
        focusTarget.set({ type: "project", projectId: project.id });
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

  {#if finishBranchTarget}
    <ConfirmModal
      title="Confirm Merge"
      message="Merge this session's branch?"
      confirmLabel="Merge"
      onConfirm={() => {
        if (finishBranchTarget) {
          const { sessionId, kind } = finishBranchTarget;
          const isCodex = kind === "codex";
          const prompt = isCodex
            ? `$the-controller-finishing-a-development-branch`
            : `/the-controller-finishing-a-development-branch`;
          if (isCodex) {
            invoke("write_to_pty", { sessionId, data: prompt }).then(() => {
              invoke("write_to_pty", { sessionId, data: "\r" });
            });
          } else {
            invoke("write_to_pty", { sessionId, data: `${prompt}\r` });
          }
        }
        finishBranchTarget = null;
      }}
      onClose={() => (finishBranchTarget = null)}
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

  /* Footer */
  .sidebar-footer {
    display: flex;
    align-items: center;
    border-top: 1px solid #313244;
    padding: 0;
  }

  .footer-spacer { flex: 1; }

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
</style>
