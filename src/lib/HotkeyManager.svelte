<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import {
    projects,
    activeSessionId,
    leaderActive,
    hotkeyAction,
    jumpMode,
    generateJumpLabels,
    JUMP_KEYS,
    sidebarVisible,
    type Project,
    type HotkeyAction,
  } from "./stores";

  let terminalHasFocus = $state(false);
  let explicitLeader = $state(false);
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

  $effect(() => {
    const unsub = projects.subscribe((value) => { projectList = value; });
    return unsub;
  });

  $effect(() => {
    const unsub = activeSessionId.subscribe((value) => { activeId = value; });
    return unsub;
  });

  // Build flattened session list from projects (sidebar visual order)
  let flatSessions: string[] = $derived(
    projectList.flatMap((p) => p.sessions.map((s) => s.id)),
  );

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

  function enterExplicitLeader() {
    explicitLeader = true;
    leaderActive.set(true);
  }

  function exitExplicitLeader() {
    explicitLeader = false;
    leaderActive.set(false);
  }

  function forwardEscape() {
    if (activeId) {
      invoke("write_to_pty", { sessionId: activeId, data: "\x1b" });
    }
  }

  function dispatchAction(action: NonNullable<HotkeyAction>) {
    hotkeyAction.set(action);
    setTimeout(() => hotkeyAction.set(null), 0);
  }

  function switchToSessionIndex(index: number) {
    if (index >= 0 && index < flatSessions.length) {
      activeSessionId.set(flatSessions[index]);
    }
  }

  function enterJumpMode() {
    if (projectList.length === 0) return;
    jumpActive = true;
    jumpPhase = "project";
    jumpProjectId = null;
    jumpBuffer = "";
    jumpLabels = generateJumpLabels(projectList.length);
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

    if (!JUMP_KEYS.includes(key)) {
      exitJumpMode();
      return;
    }

    jumpBuffer += key;

    // Check for exact match
    const matchIndex = jumpLabels.indexOf(jumpBuffer);
    if (matchIndex !== -1) {
      if (jumpPhase === "project") {
        const project = projectList[matchIndex];
        if (!project) {
          exitJumpMode();
          return;
        }
        // Always enter session phase with N+1 labels (sessions + "create new")
        jumpPhase = "session";
        jumpProjectId = project.id;
        jumpBuffer = "";
        jumpLabels = generateJumpLabels(project.sessions.length + 1);
        jumpMode.set({ phase: "session", projectId: project.id });
      } else {
        // Session phase
        const project = projectList.find((p) => p.id === jumpProjectId);
        if (project) {
          if (matchIndex < project.sessions.length) {
            activeSessionId.set(project.sessions[matchIndex].id);
          } else {
            // Last label = create new session
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

  function handleHotkey(e: KeyboardEvent): boolean {
    const key = e.key;

    // Session switching: 1-9
    if (key >= "1" && key <= "9") {
      switchToSessionIndex(parseInt(key, 10) - 1);
      return true;
    }

    switch (key) {
      case "j":
        enterJumpMode();
        return true;
      case "c":
        dispatchAction({ type: "create-session" });
        return true;
      case "x":
        dispatchAction({ type: "close-session" });
        return true;
      case "f":
        dispatchAction({ type: "open-fuzzy-finder" });
        return true;
      case "n":
        dispatchAction({ type: "open-new-project" });
        return true;
      case "h":
        dispatchAction({ type: "focus-sidebar" });
        return true;
      case "l":
        dispatchAction({ type: "focus-terminal" });
        return true;
      case "d":
        dispatchAction({ type: "delete-project" });
        return true;
      case "a":
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

    // --- Terminal focused: require Escape prefix ---
    if (inTerminal && !explicitLeader) {
      if (e.key === "Escape") {
        const now = Date.now();
        if (now - lastEscapeTime < DOUBLE_ESCAPE_MS) {
          // Double-tap Escape: forward to terminal
          forwardEscape();
          lastEscapeTime = 0;
        } else {
          // Single Escape: enter explicit leader mode
          e.stopPropagation();
          e.preventDefault();
          lastEscapeTime = now;
          enterExplicitLeader();
        }
      }
      // All other keys pass through to terminal
      return;
    }

    // --- Explicit leader mode (from terminal) ---
    if (explicitLeader) {
      e.stopPropagation();
      e.preventDefault();

      if (e.key === "Escape") {
        // Escape cancels leader, return to terminal
        exitExplicitLeader();
        return;
      }

      handleHotkey(e);
      exitExplicitLeader();
      return;
    }

    // --- Ambient leader mode (not in terminal) ---
    // Don't intercept keys when typing in input fields
    if (isEditableElementFocused()) return;

    if (e.key === "Escape") {
      leaderActive.update(v => !v);
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
