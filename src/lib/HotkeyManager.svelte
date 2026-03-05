<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import {
    projects,
    activeSessionId,
    hotkeyAction,
    jumpMode,
    generateJumpLabels,
    JUMP_KEYS,
    sidebarVisible,
    taskPanelVisible,
    archiveView,
    archivedProjects,
    focusTarget,
    expandedProjects,
    type Project,
    type HotkeyAction,
    type FocusTarget,
  } from "./stores";

  let lastEscapeTime = 0;

  const DOUBLE_ESCAPE_MS = 300;
  const DWELL_FOCUS_MS = 5000;
  let dwellTimer: ReturnType<typeof setTimeout> | null = null;

  function clearDwellTimer() {
    if (dwellTimer !== null) {
      clearTimeout(dwellTimer);
      dwellTimer = null;
    }
  }

  function startDwellTimer() {
    clearDwellTimer();
    if (isArchiveView) return;
    dwellTimer = setTimeout(() => {
      dwellTimer = null;
      dispatchAction({ type: "focus-terminal" });
    }, DWELL_FOCUS_MS);
  }

  // Jump navigation state
  let jumpActive = $state(false);
  let jumpBuffer = $state("");
  let jumpLabels: string[] = $state([]);

  // Reactive store subscriptions
  let projectList: Project[] = $state([]);
  let activeId: string | null = $state(null);
  let currentFocus: FocusTarget = $state(null);

  $effect(() => {
    const unsub = projects.subscribe((value) => { projectList = value; });
    return unsub;
  });

  $effect(() => {
    const unsub = activeSessionId.subscribe((value) => { activeId = value; });
    return unsub;
  });

  $effect(() => {
    const unsub = focusTarget.subscribe((v) => { currentFocus = v; });
    return unsub;
  });

  let isArchiveView = $state(false);
  let archivedProjectList: Project[] = $state([]);
  let expandedSet: Set<string> = $state(new Set());

  $effect(() => {
    const unsub = archiveView.subscribe((v) => { isArchiveView = v; });
    return unsub;
  });

  $effect(() => {
    const unsub = archivedProjects.subscribe((v) => { archivedProjectList = v; });
    return unsub;
  });

  $effect(() => {
    const unsub = expandedProjects.subscribe((v) => { expandedSet = v; });
    return unsub;
  });

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
      invoke("write_to_pty", { sessionId: activeId, data: "\x1b" });
    }
  }

  function focusActiveSession() {
    clearDwellTimer();
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
    hotkeyAction.set(action);
    setTimeout(() => hotkeyAction.set(null), 0);
  }

  function getJumpProjects(): Project[] {
    return isArchiveView ? archivedProjectList : projectList;
  }

  function enterJumpMode() {
    clearDwellTimer();
    const list = getJumpProjects();
    if (list.length === 0) return;
    jumpActive = true;
    jumpBuffer = "";
    jumpLabels = generateJumpLabels(list.length);
    jumpMode.set({ phase: "project" });
  }

  function exitJumpMode() {
    jumpActive = false;
    jumpBuffer = "";
    jumpLabels = [];
    jumpMode.set(null);
  }

  function handleJumpKey(key: string) {
    if (key === "Escape") {
      exitJumpMode();
      return;
    }

    if (!JUMP_KEYS.includes(key)) {
      exitJumpMode();
      return;
    }

    jumpBuffer += key;

    // Check for exact match
    const matchIndex = jumpLabels.indexOf(jumpBuffer);
    if (matchIndex !== -1) {
      const list = getJumpProjects();
      const project = list[matchIndex];
      if (project) {
        focusTarget.set({ type: "project", projectId: project.id });
      }
      exitJumpMode();
      return;
    }

    // Check if buffer is a valid prefix of any label
    const isPrefix = jumpLabels.some((l) => l.startsWith(jumpBuffer));
    if (!isPrefix) {
      exitJumpMode();
    }
  }

  type SidebarItem =
    | { type: "project"; projectId: string }
    | { type: "session"; sessionId: string; projectId: string };

  function getVisibleItems(): SidebarItem[] {
    const list = isArchiveView ? archivedProjectList : projectList;
    const result: SidebarItem[] = [];
    for (const p of list) {
      result.push({ type: "project", projectId: p.id });
      if (!expandedSet.has(p.id)) continue;
      const sessions = isArchiveView
        ? p.sessions.filter(s => s.archived)
        : p.sessions.filter(s => !s.archived);
      for (const s of sessions) {
        result.push({ type: "session", sessionId: s.id, projectId: p.id });
      }
    }
    return result;
  }

  function navigateItem(direction: 1 | -1) {
    clearDwellTimer();
    const items = getVisibleItems();
    if (items.length === 0) return;
    let idx = -1;
    if (currentFocus?.type === "session") {
      idx = items.findIndex(it => it.type === "session" && it.sessionId === currentFocus.sessionId);
    } else if (currentFocus?.type === "project") {
      idx = items.findIndex(it => it.type === "project" && it.projectId === currentFocus.projectId);
    }
    const len = items.length;
    const next = items[((idx + direction) % len + len) % len];
    if (next.type === "session") {
      if (!isArchiveView) {
        activeSessionId.set(next.sessionId);
      }
      focusTarget.set({ type: "session", sessionId: next.sessionId, projectId: next.projectId });
      startDwellTimer();
    } else {
      focusTarget.set({ type: "project", projectId: next.projectId });
    }
  }

  function navigateProject(direction: 1 | -1) {
    clearDwellTimer();
    const list = isArchiveView ? archivedProjectList : projectList;
    if (list.length === 0) return;
    let idx = -1;
    if (currentFocus?.type === "project") {
      idx = list.findIndex(p => p.id === currentFocus.projectId);
    } else if (currentFocus?.type === "session") {
      idx = list.findIndex(p => p.id === currentFocus.projectId);
    }
    const len = list.length;
    const next = list[((idx + direction) % len + len) % len];
    focusTarget.set({ type: "project", projectId: next.id });
  }

  function handleHotkey(e: KeyboardEvent): boolean {
    const key = e.key;

    switch (key) {
      case "g":
        enterJumpMode();
        return true;
      case "j":
        navigateItem(1);
        return true;
      case "k":
        navigateItem(-1);
        return true;
      case "J":
        navigateProject(1);
        return true;
      case "K":
        navigateProject(-1);
        return true;
      case "f":
        dispatchAction({ type: "open-fuzzy-finder" });
        return true;
      case "n":
        dispatchAction({ type: "open-new-project" });
        return true;
      case "d":
        if (currentFocus?.type === "session") {
          dispatchAction({ type: "delete-session", sessionId: currentFocus.sessionId, projectId: currentFocus.projectId });
        } else if (currentFocus?.type === "project") {
          dispatchAction({ type: "delete-project", projectId: currentFocus.projectId });
        } else {
          dispatchAction({ type: "delete-project" });
        }
        return true;
      case "a":
        if (isArchiveView) {
          // In archive view, a unarchives the focused item
          if (currentFocus?.type === "session") {
            dispatchAction({ type: "unarchive-session", sessionId: currentFocus.sessionId, projectId: currentFocus.projectId });
          } else if (currentFocus?.type === "project") {
            dispatchAction({ type: "unarchive-project", projectId: currentFocus.projectId });
          }
        } else if (currentFocus?.type === "session") {
          dispatchAction({ type: "archive-session", sessionId: currentFocus.sessionId, projectId: currentFocus.projectId });
        } else if (currentFocus?.type === "project") {
          dispatchAction({ type: "archive-project", projectId: currentFocus.projectId });
        } else {
          dispatchAction({ type: "archive-project" });
        }
        return true;
      case "A":
        dispatchAction({ type: "toggle-archive-view" });
        return true;
      case "c":
        if (currentFocus?.type === "project" || currentFocus?.type === "session") {
          const project = projectList.find(p => p.id === currentFocus.projectId);
          if (project) {
            dispatchAction({ type: "pick-issue-for-session", projectId: project.id, repoPath: project.repo_path });
          }
        }
        return true;
      case "x":
        if (currentFocus?.type === "project" || currentFocus?.type === "session") {
          const project = projectList.find(p => p.id === currentFocus.projectId);
          if (project) {
            dispatchAction({ type: "pick-issue-for-session", projectId: project.id, repoPath: project.repo_path, kind: "codex" });
          }
        }
        return true;
      case "m":
        if (currentFocus?.type === "session") {
          dispatchAction({ type: "merge-session", sessionId: currentFocus.sessionId, projectId: currentFocus.projectId });
        }
        return true;
      case "s":
        sidebarVisible.update(v => !v);
        return true;
      case "i":
        if (currentFocus?.type === "project" || currentFocus?.type === "session") {
          const project = projectList.find(p => p.id === currentFocus.projectId);
          if (project) {
            dispatchAction({ type: "create-issue", projectId: project.id, repoPath: project.repo_path });
          }
        }
        return true;
      case "t":
        taskPanelVisible.update(v => !v);
        return true;
      case "l":
      case "Enter":
        clearDwellTimer();
        if (currentFocus?.type === "project") {
          const next = new Set(expandedSet);
          if (next.has(currentFocus.projectId)) {
            next.delete(currentFocus.projectId);
          } else {
            next.add(currentFocus.projectId);
          }
          expandedProjects.set(next);
        } else if (currentFocus?.type === "session") {
          if (!isArchiveView) {
            activeSessionId.set(currentFocus.sessionId);
          }
          dispatchAction({ type: "focus-terminal" });
        }
        return true;
      case "?":
        dispatchAction({ type: "toggle-help" });
        return true;
      default:
        return false;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    // Ignore modifier-only keypresses
    if (["Shift", "Control", "Alt", "Meta"].includes(e.key)) return;

    // Jump mode intercepts all keys
    if (jumpActive) {
      e.stopPropagation();
      e.preventDefault();
      handleJumpKey(e.key);
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
        }
      }
      // All other keys pass through to terminal
      return;
    }

    // --- Ambient mode (not in terminal) ---
    // Allow dialog-local keyboard handlers to own key events.
    if (isDialogOpen()) return;

    // Don't intercept keys when typing in input fields
    if (isEditableElementFocused()) return;

    // Escape: check for double-tap (forward to terminal), else walk up focus hierarchy
    if (e.key === "Escape") {
      const now = Date.now();
      if (now - lastEscapeTime < DOUBLE_ESCAPE_MS) {
        // Double-tap Escape: forward to terminal and refocus it
        forwardEscape();
        lastEscapeTime = 0;
        clearDwellTimer();
        dispatchAction({ type: "focus-terminal" });
        e.stopPropagation();
        e.preventDefault();
      } else if (currentFocus?.type === "session") {
        clearDwellTimer();
        focusTarget.set({ type: "project", projectId: currentFocus.projectId });
        e.stopPropagation();
        e.preventDefault();
      }
      return;
    }

    // Try to handle as hotkey
    if (handleHotkey(e)) {
      e.stopPropagation();
      e.preventDefault();
    }
    // Unrecognized keys pass through normally
  }

  onMount(() => {
    window.addEventListener("keydown", onKeydown, { capture: true });
    return () => {
      clearDwellTimer();
      window.removeEventListener("keydown", onKeydown, { capture: true });
    };
  });
</script>
