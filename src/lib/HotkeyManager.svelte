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
    archiveView,
    archivedProjects,
    focusTarget,
    type Project,
    type HotkeyAction,
    type FocusTarget,
  } from "./stores";

  let lastEscapeTime = 0;

  const DOUBLE_ESCAPE_MS = 300;

  // Jump navigation state
  let jumpActive = $state(false);
  let jumpPhase: "project" | "session" = $state("project");
  let jumpProjectId: string | null = $state(null);
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

  $effect(() => {
    const unsub = archiveView.subscribe((v) => { isArchiveView = v; });
    return unsub;
  });

  $effect(() => {
    const unsub = archivedProjects.subscribe((v) => { archivedProjectList = v; });
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

  function forwardEscape() {
    if (activeId) {
      invoke("write_to_pty", { sessionId: activeId, data: "\x1b" });
    }
  }

  function focusActiveSession() {
    if (!activeId) return;
    const project = projectList.find((p) =>
      p.sessions.some((s) => s.id === activeId),
    );
    if (project) {
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
    const list = getJumpProjects();
    if (list.length === 0) return;
    jumpActive = true;
    jumpPhase = "project";
    jumpProjectId = null;
    jumpBuffer = "";
    jumpLabels = generateJumpLabels(list.length);
    jumpMode.set({ phase: "project" });
  }

  function exitJumpMode() {
    jumpActive = false;
    jumpPhase = "project";
    jumpProjectId = null;
    jumpBuffer = "";
    jumpLabels = [];
    jumpMode.set(null);
  }

  function handleJumpKey(key: string) {
    if (key === "Escape") {
      exitJumpMode();
      return;
    }

    // In session phase, d/a operate on the jumped-to project
    if (jumpPhase === "session" && jumpProjectId && (key === "d" || key === "a")) {
      const actionType = key === "d" ? "delete-project" : "archive-project";
      dispatchAction({ type: actionType, projectId: jumpProjectId } as NonNullable<HotkeyAction>);
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
      if (jumpPhase === "project") {
        const project = list[matchIndex];
        if (!project) {
          exitJumpMode();
          return;
        }
        const sessions = isArchiveView
          ? project.sessions.filter(s => s.archived)
          : project.sessions.filter(s => !s.archived);
        // In archive view, no "create new" option
        const labelCount = isArchiveView ? sessions.length : sessions.length + 1;
        jumpPhase = "session";
        jumpProjectId = project.id;
        jumpBuffer = "";
        jumpLabels = generateJumpLabels(labelCount);
        jumpMode.set({ phase: "session", projectId: project.id });
      } else {
        // Session phase
        const project = list.find((p) => p.id === jumpProjectId);
        if (project) {
          const sessions = isArchiveView
            ? project.sessions.filter(s => s.archived)
            : project.sessions.filter(s => !s.archived);
          if (matchIndex < sessions.length) {
            const session = sessions[matchIndex];
            if (isArchiveView) {
              dispatchAction({ type: "unarchive-session", sessionId: session.id, projectId: project.id });
            } else {
              activeSessionId.set(session.id);
              // Delay focus-terminal to let activeSessionId propagate to TerminalManager
              setTimeout(() => dispatchAction({ type: "focus-terminal" }), 50);
            }
          } else if (!isArchiveView) {
            // Last label = create new session (active view only)
            dispatchAction({ type: "create-session", projectId: project.id });
          }
        }
        exitJumpMode();
      }
      return;
    }

    // Check if buffer is a valid prefix of any label
    const isPrefix = jumpLabels.some((l) => l.startsWith(jumpBuffer));
    if (!isPrefix) {
      exitJumpMode();
    }
  }

  function getVisibleSessions(): { sessionId: string; projectId: string }[] {
    const list = isArchiveView ? archivedProjectList : projectList;
    const result: { sessionId: string; projectId: string }[] = [];
    for (const p of list) {
      const sessions = isArchiveView
        ? p.sessions.filter(s => s.archived)
        : p.sessions.filter(s => !s.archived);
      for (const s of sessions) {
        result.push({ sessionId: s.id, projectId: p.id });
      }
    }
    return result;
  }

  function navigateSession(direction: 1 | -1) {
    const sessions = getVisibleSessions();
    if (sessions.length === 0) return;
    let idx = -1;
    if (currentFocus?.type === "session") {
      idx = sessions.findIndex(s => s.sessionId === currentFocus.sessionId);
    } else if (currentFocus?.type === "project") {
      // j from project → first session of that project; k from project → last session of prev project
      const projIdx = sessions.findIndex(s => s.projectId === currentFocus.projectId);
      idx = direction === 1 ? projIdx - 1 : projIdx;
    }
    const len = sessions.length;
    const next = sessions[((idx + direction) % len + len) % len];
    activeSessionId.set(next.sessionId);
    focusTarget.set({ type: "session", sessionId: next.sessionId, projectId: next.projectId });
  }

  function navigateProject(direction: 1 | -1) {
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
        navigateSession(1);
        return true;
      case "k":
        navigateSession(-1);
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
        if (currentFocus?.type === "session") {
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
      case "s":
        sidebarVisible.update(v => !v);
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
    // Don't intercept keys when typing in input fields
    if (isEditableElementFocused()) return;

    // Escape walks up focus hierarchy: session → project (stops there)
    if (e.key === "Escape") {
      if (currentFocus?.type === "session") {
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
      window.removeEventListener("keydown", onKeydown, { capture: true });
    };
  });
</script>
