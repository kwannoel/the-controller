<script lang="ts">
  import { onMount, tick } from "svelte";
  import { fromStore, get } from "svelte/store";
  import { command } from "$lib/backend";
  import {
    projects,
    activeSessionId,
    sidebarVisible,
    workspaceMode,
    workspaceModePickerVisible,
    selectedSessionProvider,
    focusTarget,
    expandedProjects,
    dispatchHotkeyAction,
    noteEntries,
    noteFolders,
    activeNote,
    type Project,
    type HotkeyAction,
    type FocusTarget,
  } from "./stores";
  import { toggleKeystrokeVisualizer, pushKeystroke } from "./keystroke-visualizer";
  import { buildKeyMap, type CommandId } from "./commands";
  import { focusForModeSwitch } from "./focus-helpers";

  let lastEscapeTime = 0;

  const DOUBLE_ESCAPE_MS = 300;

  // Workspace mode state (Space prefix)
  let workspaceModeActive = $state(false);

  const projectsState = fromStore(projects);
  let projectList: Project[] = $derived(projectsState.current);
  const activeSessionIdState = fromStore(activeSessionId);
  let activeId: string | null = $derived(activeSessionIdState.current);
  const focusTargetState = fromStore(focusTarget);
  let currentFocus: FocusTarget = $derived(focusTargetState.current);
  const expandedProjectsState = fromStore(expandedProjects);
  let expandedSet: Set<string> = $derived(expandedProjectsState.current);
  const workspaceModeState = fromStore(workspaceMode);
  let currentMode = $derived(workspaceModeState.current);
  let keyMap = $derived(buildKeyMap(currentMode));
  const noteEntriesState = fromStore(noteEntries);
  let noteEntriesMap = $derived(noteEntriesState.current);
  const noteFoldersState = fromStore(noteFolders);
  let noteFolderList = $derived(noteFoldersState.current);
  const selectedSessionProviderState = fromStore(selectedSessionProvider);
  let currentSessionProvider = $derived(selectedSessionProviderState.current);

  // Detect if a terminal (xterm) has focus
  function isTerminalFocused(): boolean {
    const el = document.activeElement;
    if (!el) return false;
    // xterm renders a textarea for input capture
    return el.closest(".xterm") !== null;
  }

  // Detect if an input/textarea/contenteditable has focus
  function isEditableElementFocused(): boolean {
    const el = document.activeElement;
    if (!el) return false;
    if (el.tagName === "INPUT" || el.tagName === "TEXTAREA") return true;
    if ((el as HTMLElement).isContentEditable) return true;
    return false;
  }

  function isDialogOpen(): boolean {
    return document.querySelector('[role="dialog"]') !== null;
  }

  function forwardEscape() {
    if (activeId) {
      command("write_to_pty", { sessionId: activeId, data: "\x1b" });
    }
  }

  function focusActiveSession() {
    if (!activeId) return;
    const project = projectList.find((p) =>
      p.sessions.some((s) => s.id === activeId),
    );
    if (project) {
      sidebarVisible.set(true);
      focusTarget.set({ type: "session", sessionId: activeId, projectId: project.id });
    }
  }

  function dispatchAction(action: NonNullable<HotkeyAction>) {
    dispatchHotkeyAction(action);
  }

  function handleWorkspaceModeKey(key: string) {
    workspaceModeActive = false;
    workspaceModePickerVisible.set(false);
    if (key === "d") {
      workspaceMode.set("development");
      const newFocus = focusForModeSwitch(currentFocus, "development", activeId, projectList);
      if (newFocus !== currentFocus) focusTarget.set(newFocus);
      return;
    }
    if (key === "a") {
      workspaceMode.set("agents");
      const newFocus = focusForModeSwitch(currentFocus, "agents", activeId, projectList);
      if (newFocus !== currentFocus) focusTarget.set(newFocus);
      return;
    }
    if (key === "r") {
      workspaceMode.set("architecture");
      const newFocus = focusForModeSwitch(currentFocus, "architecture", activeId, projectList);
      if (newFocus !== currentFocus) focusTarget.set(newFocus);
      return;
    }
    if (key === "n") {
      workspaceMode.set("notes");
      const newFocus = focusForModeSwitch(currentFocus, "notes", activeId, projectList);
      if (newFocus !== currentFocus) focusTarget.set(newFocus);
      return;
    }
    if (key === "i") {
      workspaceMode.set("infrastructure");
      const newFocus = focusForModeSwitch(currentFocus, "infrastructure", activeId, projectList);
      if (newFocus !== currentFocus) focusTarget.set(newFocus);
      return;
    }
    // Any other key (including Escape) cancels
  }

  type SidebarItem =
    | { type: "project"; projectId: string }
    | { type: "session"; sessionId: string; projectId: string }
    | { type: "agent"; agentKind: "auto-worker" | "maintainer"; projectId: string }
    | { type: "folder"; folder: string }
    | { type: "note"; filename: string; folder: string };

  function getVisibleItems(): SidebarItem[] {
    if (currentMode === "agents") {
      const result: SidebarItem[] = [];
      for (const p of projectList) {
        result.push({ type: "project", projectId: p.id });
        if (!expandedSet.has(p.id)) continue;
        result.push({ type: "agent", agentKind: "auto-worker", projectId: p.id });
        result.push({ type: "agent", agentKind: "maintainer", projectId: p.id });
      }
      return result;
    }
    if (currentMode === "notes") {
      const result: SidebarItem[] = [];
      for (const folder of noteFolderList) {
        result.push({ type: "folder", folder });
        if (!expandedSet.has(folder)) continue;
        const notes = noteEntriesMap.get(folder) ?? [];
        for (const n of notes) {
          result.push({ type: "note", filename: n.filename, folder });
        }
      }
      return result;
    }
    if (currentMode === "infrastructure") {
      const result: SidebarItem[] = [];
      for (const p of projectList) {
        result.push({ type: "project", projectId: p.id });
      }
      return result;
    }
    const result: SidebarItem[] = [];
    for (const p of projectList) {
      result.push({ type: "project", projectId: p.id });
      if (!expandedSet.has(p.id)) continue;
      const sessions = p.sessions.filter(s => !s.auto_worker_session);
      for (const s of sessions) {
        result.push({ type: "session", sessionId: s.id, projectId: p.id });
      }
    }
    return result;
  }

  function navigateItem(direction: 1 | -1) {
    const items = getVisibleItems();
    if (items.length === 0) return;
    let idx = -1;
    if (currentFocus?.type === "session") {
      idx = items.findIndex(it => it.type === "session" && it.sessionId === currentFocus.sessionId);
    } else if (currentFocus?.type === "agent") {
      idx = items.findIndex(it => it.type === "agent" && it.projectId === currentFocus.projectId && it.agentKind === currentFocus.agentKind);
    } else if (currentFocus?.type === "folder") {
      idx = items.findIndex(it => it.type === "folder" && it.folder === currentFocus.folder);
    } else if (currentFocus?.type === "note") {
      idx = items.findIndex(it => it.type === "note" && it.folder === currentFocus.folder && it.filename === currentFocus.filename);
    } else if (currentFocus?.type === "project") {
      idx = items.findIndex(it => it.type === "project" && it.projectId === currentFocus.projectId);
    }
    const len = items.length;
    const next = items[((idx + direction) % len + len) % len];
    if (next.type === "session") {
      activeSessionId.set(next.sessionId);
      focusTarget.set({ type: "session", sessionId: next.sessionId, projectId: next.projectId });
    } else if (next.type === "agent") {
      focusTarget.set({ type: "agent", agentKind: next.agentKind, projectId: next.projectId });
    } else if (next.type === "folder") {
      focusTarget.set({ type: "folder", folder: next.folder });
    } else if (next.type === "note") {
      focusTarget.set({ type: "note", filename: next.filename, folder: next.folder });
    } else {
      focusTarget.set({ type: "project", projectId: next.projectId });
    }
  }

  function navigateProject(direction: 1 | -1) {
    if (currentMode === "notes") {
      if (noteFolderList.length === 0) return;
      const focusedFolder = currentFocus?.type === "folder" ? currentFocus.folder
        : currentFocus?.type === "note" ? currentFocus.folder
        : currentFocus?.type === "notes-editor" ? currentFocus.folder
        : null;
      let idx = -1;
      if (focusedFolder) idx = noteFolderList.indexOf(focusedFolder);
      const len = noteFolderList.length;
      const next = noteFolderList[((idx + direction) % len + len) % len];
      focusTarget.set({ type: "folder", folder: next });
      return;
    }
    if (projectList.length === 0) return;
    const focusedProjectId = currentFocus?.type === "project" || currentFocus?.type === "session" || currentFocus?.type === "agent" || currentFocus?.type === "agent-panel"
      ? currentFocus.projectId
      : null;
    let idx = -1;
    if (focusedProjectId) idx = projectList.findIndex(p => p.id === focusedProjectId);
    const len = projectList.length;
    const next = projectList[((idx + direction) % len + len) % len];
    focusTarget.set({ type: "project", projectId: next.id });
  }

  function getFocusedProject(): Project | null {
    if (currentFocus?.type === "project" || currentFocus?.type === "session" || currentFocus?.type === "agent" || currentFocus?.type === "agent-panel") {
      return projectList.find((p) => p.id === currentFocus.projectId) ?? null;
    }
    return null;
  }

  function dispatchDeleteAction() {
    if (currentFocus?.type === "folder") {
      dispatchAction({ type: "delete-folder", folder: currentFocus.folder });
      return;
    }
    if (currentFocus?.type === "session") {
      dispatchAction({ type: "delete-session", sessionId: currentFocus.sessionId, projectId: currentFocus.projectId });
      return;
    }
    if (currentFocus?.type === "project") {
      dispatchAction({ type: "delete-project", projectId: currentFocus.projectId });
      return;
    }
    dispatchAction({ type: "delete-project" });
  }

  function dispatchIssuePicker(opts?: { kind?: "claude" | "codex"; background?: boolean }) {
    const project = getFocusedProject();
    if (!project) return;
    dispatchAction({
      type: "pick-issue-for-session",
      projectId: project.id,
      repoPath: project.repo_path,
      kind: opts?.kind,
      background: opts?.background,
    });
  }

  function dispatchCreateIssue() {
    const project = getFocusedProject();
    if (!project) return;
    dispatchAction({ type: "create-issue", projectId: project.id, repoPath: project.repo_path });
  }

  function handleHotkey(key: string): boolean {
    const id = keyMap.get(key);
    if (id === undefined) return false;

    switch (id) {
      case "navigate-next":
        navigateItem(1);
        return true;
      case "navigate-prev":
        navigateItem(-1);
        return true;
      case "fuzzy-finder":
        dispatchAction({ type: "open-fuzzy-finder" });
        return true;
      case "new-project":
        dispatchAction({ type: "open-new-project" });
        return true;
      case "delete":
        dispatchDeleteAction();
        return true;
      case "create-session":
        dispatchIssuePicker({ kind: currentSessionProvider });
        return true;
      case "finish-branch":
        if (activeId) {
          const proj = projectList.find((p) => p.sessions.some((s) => s.id === activeId));
          const sess = proj?.sessions.find((s) => s.id === activeId);
          dispatchHotkeyAction({ type: "finish-branch", sessionId: activeId, kind: sess?.kind });
        }
        return true;
      case "save-prompt": {
        if (currentFocus?.type === "session") {
          dispatchAction({
            type: "save-session-prompt",
            sessionId: currentFocus.sessionId,
            projectId: currentFocus.projectId,
          });
        }
        return true;
      }
      case "load-prompt": {
        const project = getFocusedProject();
        if (project) {
          dispatchAction({ type: "pick-prompt-for-session", projectId: project.id });
        }
        return true;
      }
      case "generate-architecture": {
        const project = getFocusedProject();
        if (project) {
          dispatchAction({
            type: "generate-architecture",
            projectId: project.id,
            repoPath: project.repo_path,
          });
        }
        return true;
      }
      case "stage": {
        const stageProj = projectList.find((p) => p.staged_session !== null);
        if (stageProj) {
          dispatchHotkeyAction({ type: "unstage-session", projectId: stageProj.id });
        } else if (activeId) {
          const proj2 = projectList.find((p) => p.sessions.some((s) => s.id === activeId));
          if (proj2 && proj2.name === "the-controller") {
            dispatchHotkeyAction({ type: "stage-session", sessionId: activeId, projectId: proj2.id });
          }
        }
        return true;
      }
      case "create-issue":
        dispatchCreateIssue();
        return true;
      case "triage-untriaged":
        dispatchAction({ type: "toggle-triage-panel", category: "untriaged" });
        return true;
      case "triage-triaged":
        dispatchAction({ type: "toggle-triage-panel", category: "triaged" });
        return true;
      case "assigned-issues":
        dispatchAction({ type: "toggle-assigned-issues-panel" });
        return true;
      case "expand-collapse":
        if (currentFocus?.type === "project") {
          const next = new Set(expandedSet);
          if (next.has(currentFocus.projectId)) {
            next.delete(currentFocus.projectId);
          } else {
            next.add(currentFocus.projectId);
          }
          expandedProjects.set(next);
        } else if (currentFocus?.type === "session") {
          activeSessionId.set(currentFocus.sessionId);
          dispatchAction({ type: "focus-terminal" });
        } else if (currentFocus?.type === "agent") {
          focusTarget.set({ type: "agent-panel", agentKind: currentFocus.agentKind, projectId: currentFocus.projectId });
        } else if (currentFocus?.type === "folder") {
          const next = new Set(expandedSet);
          if (next.has(currentFocus.folder)) {
            next.delete(currentFocus.folder);
          } else {
            next.add(currentFocus.folder);
          }
          expandedProjects.set(next);
        } else if (currentFocus?.type === "note") {
          activeNote.set({ folder: currentFocus.folder, filename: currentFocus.filename });
          const vimKeys = ["o", "i", "a"];
          focusTarget.set({ type: "notes-editor", folder: currentFocus.folder, entryKey: vimKeys.includes(key) ? key : undefined });
        }
        return true;
      case "toggle-agent": {
        const agentFocus = currentFocus?.type === "agent" ? currentFocus : currentFocus?.type === "agent-panel" ? currentFocus : null;
        if (agentFocus?.agentKind === "maintainer") {
          dispatchAction({ type: "toggle-maintainer-enabled" });
        } else {
          dispatchAction({ type: "toggle-auto-worker-enabled" });
        }
        return true;
      }
      case "trigger-agent-check":
        dispatchAction({ type: "trigger-maintainer-check" });
        return true;
      case "toggle-help":
        dispatchAction({ type: "toggle-help" });
        return true;
      case "clear-agent-reports":
        dispatchAction({ type: "clear-maintainer-reports" });
        return true;
      case "create-note":
        dispatchAction({ type: "create-note" });
        return true;
      case "delete-note":
        if (currentFocus?.type === "note") {
          dispatchAction({ type: "delete-note", folder: currentFocus.folder, filename: currentFocus.filename });
        }
        return true;
      case "rename-note":
        if (currentFocus?.type === "note") {
          dispatchAction({ type: "rename-note", folder: currentFocus.folder, filename: currentFocus.filename });
        } else if (currentFocus?.type === "folder") {
          dispatchAction({ type: "rename-folder", folder: currentFocus.folder });
        }
        return true;
      case "duplicate-note":
        if (currentFocus?.type === "note") {
          dispatchAction({ type: "duplicate-note", folder: currentFocus.folder, filename: currentFocus.filename });
        }
        return true;
      case "toggle-note-preview":
        dispatchAction({ type: "toggle-note-preview" });
        return true;
      case "toggle-maintainer-view":
        dispatchAction({ type: "toggle-maintainer-view" });
        return true;
      case "deploy-project": {
        const project = getFocusedProject();
        if (project) {
          dispatchAction({ type: "deploy-project", projectId: project.id, repoPath: project.repo_path });
        }
        return true;
      }
      case "rollback-deploy": {
        const project = getFocusedProject();
        if (project) {
          dispatchAction({ type: "rollback-deploy", projectId: project.id });
        }
        return true;
      }
      default: {
        const _exhaustive: never = id;
        return false;
      }
    }
  }

  function onKeydown(e: KeyboardEvent) {
    // Ignore modifier-only keypresses
    if (["Shift", "Control", "Alt", "Meta"].includes(e.key)) return;

    // Cmd+S/Cmd+Shift+S: full window screenshot (shift = preview)
    // Cmd+D/Cmd+Shift+D: cropped screenshot (shift = preview)
    if (e.metaKey && (e.key === "s" || e.key === "d")) {
      e.stopPropagation();
      e.preventDefault();
      dispatchAction({
        type: "screenshot-to-session",
        preview: e.shiftKey,
        cropped: e.key === "d",
      });
      pushKeystroke("⌘" + e.key.toUpperCase());
      return;
    }

    // Cmd+K: toggle keystroke visualizer
    if (e.metaKey && e.key === "k") {
      e.stopPropagation();
      e.preventDefault();
      toggleKeystrokeVisualizer();
      return;
    }

    // Cmd+T: toggle foreground session provider
    if (e.metaKey && e.key === "t") {
      if (isDialogOpen()) return;
      if (isEditableElementFocused() && !isTerminalFocused()) return;
      e.stopPropagation();
      e.preventDefault();
      selectedSessionProvider.update((provider) => provider === "claude" ? "codex" : "claude");
      pushKeystroke("⌘T");
      return;
    }

    // Workspace mode intercepts all keys
    if (workspaceModeActive) {
      e.stopPropagation();
      e.preventDefault();
      handleWorkspaceModeKey(e.key);
      pushKeystroke("␣" + e.key);
      return;
    }

    const inTerminal = isTerminalFocused();

    // --- Terminal focused: Escape moves focus to sidebar session ---
    if (inTerminal) {
      if (e.key === "Escape") {
        const now = Date.now();
        if (now - lastEscapeTime < DOUBLE_ESCAPE_MS) {
          // Double-tap Escape: forward to terminal
          forwardEscape();
          lastEscapeTime = 0;
        } else {
          // Single Escape: move focus to active session in sidebar
          e.stopPropagation();
          e.preventDefault();
          lastEscapeTime = now;
          focusActiveSession();
          pushKeystroke("Esc");
        }
      }
      // All other keys pass through to terminal
      return;
    }

    // --- Ambient mode (not in terminal) ---
    // Allow dialog-local keyboard handlers to own key events.
    if (isDialogOpen()) return;

    // Don't intercept keys when typing in input fields or the notes code editor
    if (isEditableElementFocused()) return;
    if (currentFocus?.type === "notes-editor") return;

    // Escape: check for double-tap (forward to terminal), else walk up focus hierarchy
    if (e.key === "Escape") {
      const now = Date.now();
      if (now - lastEscapeTime < DOUBLE_ESCAPE_MS) {
        // Double-tap Escape: forward to terminal and refocus it
        forwardEscape();
        lastEscapeTime = 0;
        dispatchAction({ type: "focus-terminal" });
        e.stopPropagation();
        e.preventDefault();
      } else if (currentFocus?.type === "note") {
        focusTarget.set({ type: "folder", folder: currentFocus.folder });
        e.stopPropagation();
        e.preventDefault();
        pushKeystroke("Esc");
      } else if (currentFocus?.type === "session") {
        focusTarget.set({ type: "project", projectId: currentFocus.projectId });
        e.stopPropagation();
        e.preventDefault();
        pushKeystroke("Esc");
      } else if (currentFocus?.type === "agent-panel") {
        dispatchAction({ type: "agent-panel-escape" });
        e.stopPropagation();
        e.preventDefault();
        pushKeystroke("Esc");
      } else if (currentFocus?.type === "agent") {
        focusTarget.set({ type: "project", projectId: currentFocus.projectId });
        e.stopPropagation();
        e.preventDefault();
        pushKeystroke("Esc");
      }
      return;
    }

    // Space: workspace mode picker
    if (e.key === " ") {
      e.stopPropagation();
      e.preventDefault();
      workspaceModeActive = true;
      workspaceModePickerVisible.set(true);
      pushKeystroke("␣");
      return;
    }

    // Agent panel focused: intercept navigation keys
    if (currentFocus?.type === "agent-panel") {
      if (e.key === "j" || e.key === "k") {
        e.stopPropagation();
        e.preventDefault();
        dispatchAction({ type: "agent-panel-navigate", direction: e.key === "j" ? 1 : -1 });
        pushKeystroke(e.key);
        return;
      }
      if (e.key === "l" || e.key === "Enter") {
        e.stopPropagation();
        e.preventDefault();
        dispatchAction({ type: "agent-panel-select" });
        pushKeystroke(e.key);
        return;
      }
      if (e.key === "o") {
        e.stopPropagation();
        e.preventDefault();
        dispatchAction({ type: "open-issue-in-browser" });
        pushKeystroke(e.key);
        return;
      }
    }

    // Try to handle as hotkey
    if (handleHotkey(e.key)) {
      e.stopPropagation();
      e.preventDefault();
      pushKeystroke(e.key);
    }
    // Unrecognized keys pass through normally
  }

  onMount(() => {
    window.addEventListener("keydown", onKeydown, { capture: true });
    return () => {
      window.removeEventListener("keydown", onKeydown, { capture: true });
    };
  });
</script>
