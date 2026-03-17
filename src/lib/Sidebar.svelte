<script lang="ts">
  import { fromStore } from "svelte/store";
  import { command, listen } from "$lib/backend";
  import { refreshProjectsFromBackend } from "./project-listing";
  import { projects, activeSessionId, sessionStatuses, maintainerStatuses, maintainerErrors, autoWorkerStatuses, hotkeyAction, showKeyHints, focusTarget, expandedProjects, focusTerminalSoon, workspaceMode, activeNote, noteEntries, noteFolders, selectedSessionProvider, type CorruptProjectEntry, type Project, type ProjectInventory, type FocusTarget, type SessionStatus, type MaintainerStatus, type AutoWorkerStatus, type NoteEntry } from "./stores";
  import { showToast } from "./toast";
  import { focusAfterSessionDelete, focusAfterProjectDelete } from "./focus-helpers";
  import { sendFinishBranchPrompt } from "./finish-branch";
  import FuzzyFinder from "./FuzzyFinder.svelte";
  import NewProjectModal from "./NewProjectModal.svelte";
  import DeleteProjectModal from "./DeleteProjectModal.svelte";
  import ConfirmModal from "./ConfirmModal.svelte";
  import DeleteSessionModal from "./DeleteSessionModal.svelte";
  import ProjectTree from "./sidebar/ProjectTree.svelte";
  import AgentTree from "./sidebar/AgentTree.svelte";
  import NotesTree from "./sidebar/NotesTree.svelte";
  import NewNoteModal from "./NewNoteModal.svelte";
  import NewFolderModal from "./NewFolderModal.svelte";
  import RenameNoteModal from "./RenameNoteModal.svelte";

  let sidebarEl: HTMLElement | undefined = $state();
  const showKeyHintsState = fromStore(showKeyHints);
  let showFuzzyFinder = $state(false);
  let showNewProjectModal = $state(false);
  const expandedProjectsState = fromStore(expandedProjects);
  let expandedProjectSet: Set<string> = $derived(expandedProjectsState.current);
  let deleteTarget: Project | null = $state(null);
  let deleteSessionTarget: { sessionId: string; projectId: string; label: string } | null = $state(null);
  let mergeSessionTarget: { sessionId: string; projectId: string; label: string } | null = $state(null);
  let mergeInProgress = $state(false);
  let staging = $state(false);
  let finishBranchTarget: { sessionId: string; kind?: "claude" | "codex" } | null = $state(null);
  const workspaceModeState = fromStore(workspaceMode);
  let currentMode = $derived(workspaceModeState.current);
  const selectedSessionProviderState = fromStore(selectedSessionProvider);
  let currentSessionProvider = $derived(selectedSessionProviderState.current);
  let currentSessionProviderLabel = $derived(currentSessionProvider === "codex" ? "Codex" : "Claude");
  let deleteNoteTarget: { folder: string; filename: string } | null = $state(null);
  let renameNoteTarget: { folder: string; filename: string } | null = $state(null);
  let showNewNoteModal = $state(false);
  let showNewFolderModal = $state(false);
  let renameFolderTarget: string | null = $state(null);
  let deleteFolderTarget: string | null = $state(null);
  const activeNoteState = fromStore(activeNote);
  const noteFoldersState = fromStore(noteFolders);
  let folderList: string[] = $derived(noteFoldersState.current);
  const projectsState = fromStore(projects);
  let projectList: Project[] = $derived(projectsState.current);
  const activeSessionIdState = fromStore(activeSessionId);
  let activeSession: string | null = $derived(activeSessionIdState.current);
  const sessionStatusesState = fromStore(sessionStatuses);
  let statuses: Map<string, SessionStatus> = $derived(sessionStatusesState.current);
  const idleTimers = new Map<string, ReturnType<typeof setTimeout>>();
  const IDLE_DEBOUNCE_MS = 1500;
  let surfacedCorruptProjectWarnings = $state(new Set<string>());

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
    } else if (currentFocus?.type === "agent") {
      if (!expandedProjectSet.has(currentFocus.projectId)) {
        const next = new Set(expandedProjectSet);
        next.add(currentFocus.projectId);
        expandedProjects.set(next);
      }
      if (sidebarEl) {
        requestAnimationFrame(() => {
          const el = sidebarEl?.querySelector<HTMLElement>(`[data-agent-id="${currentFocus.projectId}:${currentFocus.agentKind}"]`);
          if (el) el.focus();
        });
      }
    } else if (currentFocus?.type === "agent-panel") {
      // Blur sidebar element so visual focus moves to the panel
      if (document.activeElement instanceof HTMLElement && sidebarEl?.contains(document.activeElement)) {
        document.activeElement.blur();
      }
    } else if (currentFocus?.type === "folder") {
      if (sidebarEl) {
        requestAnimationFrame(() => {
          const el = sidebarEl?.querySelector<HTMLElement>(`[data-folder-id="${currentFocus.folder}"]`);
          if (el) el.focus();
        });
      }
    } else if (currentFocus?.type === "note") {
      if (!expandedProjectSet.has(currentFocus.folder)) {
        const next = new Set(expandedProjectSet);
        next.add(currentFocus.folder);
        expandedProjects.set(next);
      }
      if (sidebarEl) {
        requestAnimationFrame(() => {
          const el = sidebarEl?.querySelector<HTMLElement>(`[data-note-id="${currentFocus.folder}:${currentFocus.filename}"]`);
          if (el) el.focus();
        });
      }
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
          if (project) createSession(project.id, action.kind);
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
        case "e2e-eval": {
          const skillCmd = action.kind === "codex"
            ? "$the-controller-e2e-eval"
            : "/the-controller-e2e-eval";
          if (action.kind === "codex") {
            command("write_to_pty", { sessionId: action.sessionId, data: skillCmd })
              .then(() => command("send_raw_to_pty", { sessionId: action.sessionId, data: "\r" }));
          } else {
            command("write_to_pty", { sessionId: action.sessionId, data: `${skillCmd}\r` });
          }
          break;
        }
        case "stage-session": {
          stageSession(action.projectId, action.sessionId);
          break;
        }
        case "unstage-session": {
          unstageSession(action.projectId, action.sessionId);
          break;
        }
        case "create-note": {
          showNewNoteModal = true;
          break;
        }
        case "create-folder": {
          showNewFolderModal = true;
          break;
        }
        case "delete-note": {
          deleteNoteTarget = { folder: action.folder, filename: action.filename };
          break;
        }
        case "rename-note": {
          renameNoteTarget = { folder: action.folder, filename: action.filename };
          break;
        }
        case "duplicate-note": {
          handleDuplicateNote(action.folder, action.filename);
          break;
        }
        case "rename-folder": {
          renameFolderTarget = action.folder;
          break;
        }
        case "delete-folder": {
          deleteFolderTarget = action.folder;
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
    command<string[]>("list_folders", {}).then(folders => {
      noteFolders.set(folders);
    });
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

    for (const project of projectList) {
      for (const session of project.sessions) {
        unlisteners.push(listen<string>(`session-status-changed:${session.id}`, () => {
          markSession(session.id, "exited");
        }));

        // Cleanup: backend already deleted the session and worktree, just refresh.
        unlisteners.push(listen<string>(`session-cleanup:${session.id}`, () => {
          const nextFocus = focusAfterSessionDelete(projectList, project.id, session.id);
          clearSessionTracking(session.id);
          activeSessionId.update(current => {
            if (current !== session.id) return current;
            if (nextFocus?.type === "session") return nextFocus.sessionId;
            return null;
          });
          focusTarget.set(nextFocus);
          loadProjects();
        }));

        // Hook-based status: precise idle/working from Claude Code hooks.
        // Debounce idle transitions to avoid flickering between tool calls
        // (Stop hook fires after each assistant turn, even mid-task).
        unlisteners.push(listen<string>(`session-status-hook:${session.id}`, (payload) => {
          const status = payload as SessionStatus;
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
        }));
      }

      unlisteners.push(listen<string>(`maintainer-status:${project.id}`, (payload) => {
        maintainerStatuses.update(m => {
          const next = new Map(m);
          next.set(project.id, payload as MaintainerStatus);
          return next;
        });
        // Clear error when status changes to non-error
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
      // Don't clear idle timers here — pending idle transitions must complete.
      // Individual session timers are cleaned up by clearSessionTracking().
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

  async function createSession(projectId: string, kind?: string) {
    try {
      const sessionId: string = await command("create_session", {
        projectId,
        kind: kind ?? currentSessionProvider,
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

  async function maybeRemoveInProgressLabel(projectId: string, sessionId: string) {
    const project = projectList.find((p) => p.id === projectId);
    const session = project?.sessions.find((s) => s.id === sessionId);
    if (session?.github_issue && project) {
      command("remove_github_label", {
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

  async function closeSession(projectId: string, sessionId: string, deleteWorktree: boolean) {
    try {
      const nextFocus = focusAfterSessionDelete(projectList, projectId, sessionId);

      await maybeRemoveInProgressLabel(projectId, sessionId);

      await command("close_session", { projectId, sessionId, deleteWorktree });
      clearSessionTracking(sessionId);
      activeSessionId.update(current => {
        if (current !== sessionId) return current;
        if (nextFocus?.type === "session") return nextFocus.sessionId;
        return null;
      });
      focusTarget.set(nextFocus);
      await loadProjects();
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function stageSession(projectId: string, sessionId: string) {
    if (staging) return;
    staging = true;
    activeSessionId.set(sessionId);
    focusTerminalSoon();

    const unlistenStatus = listen<string>("staging-status", (payload) => {
      showToast(payload, "info");
    });

    try {
      await command("stage_session", { projectId, sessionId });
      await loadProjects();
      const session = projectList
        .find((p) => p.id === projectId)
        ?.sessions.find((s) => s.id === sessionId);
      showToast(`Staged ${session?.label ?? "session"} — launching on separate port`, "info");
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      staging = false;
      unlistenStatus?.();
    }
  }

  async function unstageSession(projectId: string, sessionId: string) {
    try {
      await command("unstage_session", { projectId, sessionId });
      await loadProjects();
      showToast("Unstaged — stopped separate instance", "info");
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
    const unlistenStatus = listen<string>("merge-status", (payload) => {
      showToast(payload, "info");
    });

    try {
      const result: { type: string; url?: string } = await command("merge_session_branch", { projectId, sessionId });
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

  async function handleCreateNote(title: string, folder: string) {
    showNewNoteModal = false;
    try {
      const filename: string = await command("create_note", { folder, title });
      const notes = await command<NoteEntry[]>("list_notes", { folder });
      noteEntries.update(m => { const next = new Map(m); next.set(folder, notes); return next; });
      expandedProjects.update(s => { const next = new Set(s); next.add(folder); return next; });
      activeNote.set({ folder, filename });
      focusTarget.set({ type: "notes-editor", folder });
      const folders = await command<string[]>("list_folders", {});
      noteFolders.set(folders);
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function handleCreateFolder(name: string) {
    showNewFolderModal = false;
    try {
      await command("create_folder", { name });
      const folders = await command<string[]>("list_folders", {});
      noteFolders.set(folders);
      expandedProjects.update(s => { const next = new Set(s); next.add(name); return next; });
      focusTarget.set({ type: "folder", folder: name });
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function handleDeleteNote(folder: string, filename: string) {
    try {
      await command("delete_note", { folder, filename });
      const notes = await command<NoteEntry[]>("list_notes", { folder });
      noteEntries.update(m => { const next = new Map(m); next.set(folder, notes); return next; });
      const an = activeNoteState.current;
      if (an?.folder === folder && an?.filename === filename) {
        activeNote.set(null);
      }
      focusTarget.set({ type: "folder", folder });
      showToast("Note deleted", "info");
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function handleRenameNote(folder: string, oldName: string, newName: string) {
    try {
      const newFilename: string = await command("rename_note", { folder, oldName, newName });
      const notes = await command<NoteEntry[]>("list_notes", { folder });
      noteEntries.update(m => { const next = new Map(m); next.set(folder, notes); return next; });
      const an = activeNoteState.current;
      if (an?.folder === folder && an?.filename === oldName) {
        activeNote.set({ folder, filename: newFilename });
      }
      focusTarget.set({ type: "note", filename: newFilename, folder });
      showToast("Note renamed", "info");
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function handleDuplicateNote(folder: string, filename: string) {
    try {
      const newFilename: string = await command("duplicate_note", { folder, filename });
      const notes = await command<NoteEntry[]>("list_notes", { folder });
      noteEntries.update(m => { const next = new Map(m); next.set(folder, notes); return next; });
      activeNote.set({ folder, filename: newFilename });
      focusTarget.set({ type: "note", filename: newFilename, folder });
      showToast("Note duplicated", "info");
      renameNoteTarget = { folder, filename: newFilename };
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function handleRenameFolder(oldName: string, newName: string) {
    try {
      await command("rename_folder", { oldName, newName });
      const folders = await command<string[]>("list_folders", {});
      noteFolders.set(folders);
      expandedProjects.update(s => {
        const next = new Set(s);
        if (next.has(oldName)) { next.delete(oldName); next.add(newName); }
        return next;
      });
      noteEntries.update(m => {
        const next = new Map(m);
        const entries = next.get(oldName);
        if (entries) { next.delete(oldName); next.set(newName, entries); }
        return next;
      });
      const an = activeNoteState.current;
      if (an?.folder === oldName) {
        activeNote.set({ folder: newName, filename: an.filename });
      }
      focusTarget.set({ type: "folder", folder: newName });
      showToast("Folder renamed", "info");
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function handleDeleteFolder(folder: string) {
    try {
      await command("delete_folder", { name: folder, force: true });
      const folders = await command<string[]>("list_folders", {});
      noteFolders.set(folders);
      noteEntries.update((m) => { const next = new Map(m); next.delete(folder); return next; });
      expandedProjects.update((s) => { const next = new Set(s); next.delete(folder); return next; });
      const an = activeNoteState.current;
      if (an?.folder === folder) {
        activeNote.set(null);
      }
      showToast("Folder deleted", "info");
    } catch (e) {
      showToast(String(e), "error");
    }
  }


</script>

<aside class="sidebar" bind:this={sidebarEl}>
  <div class="sidebar-header">
    <h2>{{ development: "Development", agents: "Agents", architecture: "Architecture", notes: "Notes", infrastructure: "Infrastructure", voice: "Voice" }[currentMode]}</h2>
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
    {:else if currentMode === "notes"}
      <NotesTree
        folders={folderList}
        expandedFolderSet={expandedProjectSet}
        {currentFocus}
        onToggleFolder={toggleProject}
        onFolderFocus={(folder) => {
          focusTarget.set({ type: "folder", folder });
        }}
        onNoteFocus={(filename, folder) => {
          focusTarget.set({ type: "note", filename, folder });
        }}
        onNoteSelect={(filename, folder) => {
          activeNote.set({ folder, filename });
          focusTarget.set({ type: "notes-editor", folder });
        }}
      />
    {:else}
      <ProjectTree
        projects={projectList}
        {expandedProjectSet}
        {activeSession}
        {currentFocus}
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
    <div class="footer-left">
      <div class="provider-indicator">Provider: {currentSessionProviderLabel}</div>
    </div>
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
          const project = await command<Project>("load_project", { name: entry.name, repoPath: entry.path });
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
      isArchived={false}
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
      onConfirm={async () => {
        if (finishBranchTarget) {
          const { sessionId, kind } = finishBranchTarget;
          await sendFinishBranchPrompt(command, sessionId, kind);
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
        const nextFocus = focusAfterProjectDelete(projectList, deleteTarget!.id, expandedProjectSet);
        activeSessionId.update(current => {
          if (deleteTarget!.sessions.some(s => s.id === current)) return nextFocus?.type === "session" ? nextFocus.sessionId : null;
          return current;
        });
        deleteTarget = null;
        await loadProjects();
        focusTarget.set(nextFocus);
      }}
      onClose={() => (deleteTarget = null)}
    />
  {/if}

  {#if showNewNoteModal}
    <NewNoteModal
      folders={folderList}
      onSubmit={handleCreateNote}
      onClose={() => { showNewNoteModal = false; }}
    />
  {/if}

  {#if showNewFolderModal}
    <NewFolderModal
      onSubmit={handleCreateFolder}
      onClose={() => { showNewFolderModal = false; }}
    />
  {/if}

  {#if deleteNoteTarget}
    <ConfirmModal
      title="Delete Note"
      message={`Delete "${deleteNoteTarget.filename.replace(/\.md$/, "")}"?`}
      confirmLabel="Delete"
      onConfirm={() => {
        if (deleteNoteTarget) handleDeleteNote(deleteNoteTarget.folder, deleteNoteTarget.filename);
        deleteNoteTarget = null;
      }}
      onClose={() => (deleteNoteTarget = null)}
    />
  {/if}

  {#if renameNoteTarget}
    <RenameNoteModal
      currentName={renameNoteTarget.filename}
      onSubmit={(newName) => {
        if (renameNoteTarget) handleRenameNote(renameNoteTarget.folder, renameNoteTarget.filename, newName);
        renameNoteTarget = null;
      }}
      onClose={() => { renameNoteTarget = null; }}
    />
  {/if}

  {#if deleteFolderTarget}
    <ConfirmModal
      title="Delete Folder"
      message={`Delete folder "${deleteFolderTarget}" and all its notes?`}
      confirmLabel="Delete"
      onConfirm={() => {
        if (deleteFolderTarget) handleDeleteFolder(deleteFolderTarget);
        deleteFolderTarget = null;
      }}
      onClose={() => (deleteFolderTarget = null)}
    />
  {/if}

  {#if renameFolderTarget}
    <RenameNoteModal
      currentName={renameFolderTarget}
      onSubmit={(newName) => {
        if (renameFolderTarget) handleRenameFolder(renameFolderTarget, newName);
        renameFolderTarget = null;
      }}
      onClose={() => { renameFolderTarget = null; }}
    />
  {/if}
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

  /* Footer */
  .sidebar-footer {
    display: flex;
    align-items: center;
    border-top: 1px solid var(--border-default);
    padding: 0;
  }

  .footer-left {
    flex: 1;
    min-width: 0;
  }

  .provider-indicator {
    border-top: 1px solid var(--border-default);
    color: var(--text-secondary);
    font-size: 11px;
    letter-spacing: 0.02em;
    padding: 6px 12px 7px;
    text-transform: uppercase;
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
