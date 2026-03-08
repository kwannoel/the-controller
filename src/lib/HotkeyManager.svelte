<script lang="ts">
  import { onMount } from "svelte";
  import { fromStore } from "svelte/store";
  import { invoke } from "@tauri-apps/api/core";
  import {
    projects,
    activeSessionId,
    jumpMode,
    generateJumpLabels,
    JUMP_KEYS,
    sidebarVisible,

    maintainerPanelVisible,
    archiveView,
    archivedProjects,
    focusTarget,
    expandedProjects,
    dispatchHotkeyAction,
    type Project,
    type HotkeyAction,
    type FocusTarget,
  } from "./stores";
  import { toggleKeystrokeVisualizer, pushKeystroke } from "./keystroke-visualizer";

  let lastEscapeTime = 0;

  const DOUBLE_ESCAPE_MS = 300;

  // Jump navigation state
  let jumpActive = $state(false);
  let jumpBuffer = $state("");
  let jumpLabels: string[] = $state([]);

  const projectsState = fromStore(projects);
  let projectList: Project[] = $derived(projectsState.current);
  const activeSessionIdState = fromStore(activeSessionId);
  let activeId: string | null = $derived(activeSessionIdState.current);
  const focusTargetState = fromStore(focusTarget);
  let currentFocus: FocusTarget = $derived(focusTargetState.current);
  const archiveViewState = fromStore(archiveView);
  let isArchiveView = $derived(archiveViewState.current);
  const archivedProjectsState = fromStore(archivedProjects);
  let archivedProjectList: Project[] = $derived(archivedProjectsState.current);
  const expandedProjectsState = fromStore(expandedProjects);
  let expandedSet: Set<string> = $derived(expandedProjectsState.current);
  const maintainerPanelVisibleState = fromStore(maintainerPanelVisible);
  let isMaintainerPanelVisible = $derived(maintainerPanelVisibleState.current);

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

  function getJumpProjects(): Project[] {
    return isArchiveView ? archivedProjectList : projectList;
  }

  function enterJumpMode() {
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
    } else {
      focusTarget.set({ type: "project", projectId: next.projectId });
    }
  }

  function navigateProject(direction: 1 | -1) {
    const list = isArchiveView ? archivedProjectList : projectList;
    if (list.length === 0) return;
    const focusedProjectId = currentFocus?.type === "project" || currentFocus?.type === "session"
      ? currentFocus.projectId
      : null;
    let idx = -1;
    if (focusedProjectId) idx = list.findIndex(p => p.id === focusedProjectId);
    const len = list.length;
    const next = list[((idx + direction) % len + len) % len];
    focusTarget.set({ type: "project", projectId: next.id });
  }

  function getFocusedProject(): Project | null {
    if (currentFocus?.type !== "project" && currentFocus?.type !== "session") return null;
    return projectList.find((p) => p.id === currentFocus.projectId) ?? null;
  }

  function dispatchDeleteAction() {
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

  function dispatchArchiveAction() {
    if (isArchiveView) {
      if (currentFocus?.type === "session") {
        dispatchAction({ type: "unarchive-session", sessionId: currentFocus.sessionId, projectId: currentFocus.projectId });
      } else if (currentFocus?.type === "project") {
        dispatchAction({ type: "unarchive-project", projectId: currentFocus.projectId });
      }
      return;
    }

    if (currentFocus?.type === "session") {
      dispatchAction({ type: "archive-session", sessionId: currentFocus.sessionId, projectId: currentFocus.projectId });
    } else if (currentFocus?.type === "project") {
      dispatchAction({ type: "archive-project", projectId: currentFocus.projectId });
    } else {
      dispatchAction({ type: "archive-project" });
    }
  }

  function dispatchIssuePicker(opts?: { kind?: string; background?: boolean }) {
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
        dispatchDeleteAction();
        return true;
      case "a":
        dispatchArchiveAction();
        return true;
      case "A":
        dispatchAction({ type: "toggle-archive-view" });
        return true;
      case "c":
        dispatchIssuePicker();
        return true;
      case "x":
        dispatchIssuePicker({ kind: "codex" });
        return true;
      case "C":
        dispatchIssuePicker({ background: true });
        return true;
      case "X":
        dispatchIssuePicker({ kind: "codex", background: true });
        return true;
      case "m":
        if (activeId) {
          const proj = projectList.find((p) => p.sessions.some((s) => s.id === activeId));
          const sess = proj?.sessions.find((s) => s.id === activeId);
          dispatchHotkeyAction({ type: "finish-branch", sessionId: activeId, kind: sess?.kind });
        }
        return true;
      case "s":
        sidebarVisible.update(v => !v);
        return true;
      case "i":
        dispatchCreateIssue();
        return true;
      case "t":
        dispatchAction({ type: "toggle-triage-panel", category: "untriaged" });
        return true;
      case "T":
        dispatchAction({ type: "toggle-triage-panel", category: "triaged" });
        return true;
      case "l":
      case "Enter":
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
      case "o":
        if (isMaintainerPanelVisible && getFocusedProject()) {
          dispatchAction({ type: "toggle-maintainer-enabled" });
          return true;
        }
        return false;
      case "r":
        if (isMaintainerPanelVisible) {
          dispatchAction({ type: "trigger-maintainer-check" });
          return true;
        }
        return false;
      case "b":
        dispatchAction({ type: "toggle-maintainer-panel" });
        return true;
      // Cmd+S/D (screenshot) is handled earlier in onKeydown
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

    // Jump mode intercepts all keys
    if (jumpActive) {
      e.stopPropagation();
      e.preventDefault();
      handleJumpKey(e.key);
      pushKeystroke(e.key);
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

    // Don't intercept keys when typing in input fields
    if (isEditableElementFocused()) return;

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
      } else if (currentFocus?.type === "session") {
        focusTarget.set({ type: "project", projectId: currentFocus.projectId });
        e.stopPropagation();
        e.preventDefault();
        pushKeystroke("Esc");
      }
      return;
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
