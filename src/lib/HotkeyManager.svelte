<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import {
    projects,
    activeSessionId,
    leaderActive,
    hotkeyAction,
    type Project,
    type HotkeyAction,
  } from "./stores";

  type LeaderState = "idle" | "leader";

  let leaderState: LeaderState = $state("idle");
  let lastEscapeTime = 0;

  const DOUBLE_ESCAPE_MS = 300;

  // Reactive store subscriptions
  let projectList: Project[] = $state([]);
  let activeId: string | null = $state(null);

  projects.subscribe((value) => {
    projectList = value;
  });

  activeSessionId.subscribe((value) => {
    activeId = value;
  });

  // Build flattened session list from projects (sidebar visual order)
  let flatSessions: string[] = $derived(
    projectList.flatMap((p) => p.sessions.map((s) => s.id)),
  );

  function enterLeader() {
    leaderState = "leader";
    leaderActive.set(true);
    // No timeout — leader stays active until a key is pressed
  }

  function resetToIdle() {
    leaderState = "idle";
    leaderActive.set(false);
  }

  function forwardEscape() {
    if (activeId) {
      invoke("write_to_pty", { sessionId: activeId, data: "\x1b" });
    }
  }

  function dispatchAction(action: NonNullable<HotkeyAction>) {
    hotkeyAction.set(action);
    // Reset the action after a tick so consumers can react to each dispatch
    setTimeout(() => hotkeyAction.set(null), 0);
  }

  function switchToSessionIndex(index: number) {
    if (index >= 0 && index < flatSessions.length) {
      activeSessionId.set(flatSessions[index]);
    }
  }

  function switchRelative(delta: number) {
    if (flatSessions.length === 0) return;
    if (!activeId) {
      activeSessionId.set(flatSessions[0]);
      return;
    }
    const currentIndex = flatSessions.indexOf(activeId);
    if (currentIndex === -1) {
      activeSessionId.set(flatSessions[0]);
      return;
    }
    const nextIndex =
      (currentIndex + delta + flatSessions.length) % flatSessions.length;
    activeSessionId.set(flatSessions[nextIndex]);
  }

  function handleLeaderKey(e: KeyboardEvent): boolean {
    const key = e.key;

    // Session switching: 1-9
    if (key >= "1" && key <= "9") {
      const index = parseInt(key, 10) - 1;
      switchToSessionIndex(index);
      return true;
    }

    switch (key) {
      case "j":
        switchRelative(1);
        return true;
      case "k":
        switchRelative(-1);
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
      case "J":
        dispatchAction({ type: "next-project" });
        return true;
      case "K":
        dispatchAction({ type: "prev-project" });
        return true;
      case "?":
        dispatchAction({ type: "toggle-help" });
        return true;
      case "Escape":
        // Double-escape: forward to terminal
        forwardEscape();
        return true;
      default:
        return false;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (leaderState === "idle") {
      if (e.key === "Escape") {
        const now = Date.now();
        if (now - lastEscapeTime < DOUBLE_ESCAPE_MS) {
          // Double-tap Escape: forward to terminal immediately
          forwardEscape();
          lastEscapeTime = 0;
        } else {
          // First Escape: enter leader mode
          e.stopPropagation();
          e.preventDefault();
          lastEscapeTime = now;
          enterLeader();
        }
      }
      return;
    }

    if (leaderState === "leader") {
      // Ignore modifier-only keypresses
      if (["Shift", "Control", "Alt", "Meta"].includes(e.key)) return;

      e.stopPropagation();
      e.preventDefault();

      handleLeaderKey(e);
      resetToIdle();
    }
  }

  onMount(() => {
    window.addEventListener("keydown", onKeydown, { capture: true });
    return () => {
      window.removeEventListener("keydown", onKeydown, { capture: true });
    };
  });
</script>
