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
  let timeoutId: ReturnType<typeof setTimeout> | null = $state(null);

  const LEADER_TIMEOUT_MS = 300;

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

  function clearLeaderTimeout() {
    if (timeoutId !== null) {
      clearTimeout(timeoutId);
      timeoutId = null;
    }
  }

  function enterLeader() {
    leaderState = "leader";
    leaderActive.set(true);
    timeoutId = setTimeout(() => {
      // Timeout: forward Escape to terminal and return to idle
      forwardEscape();
      resetToIdle();
    }, LEADER_TIMEOUT_MS);
  }

  function resetToIdle() {
    clearLeaderTimeout();
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
      // No active session, pick the first one
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
      case "n":
        switchRelative(1);
        return true;
      case "p":
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
      case "N":
        dispatchAction({ type: "open-new-project" });
        return true;
      case "h":
        dispatchAction({ type: "focus-sidebar" });
        return true;
      case "l":
        dispatchAction({ type: "focus-terminal" });
        return true;
      case "j":
        dispatchAction({ type: "next-project" });
        return true;
      case "k":
        dispatchAction({ type: "prev-project" });
        return true;
      case "?":
        dispatchAction({ type: "toggle-help" });
        return true;
      case "Escape":
        // Escape again cancels leader, forward to terminal
        forwardEscape();
        return true;
      default:
        // Unrecognized key: cancel leader mode, don't forward escape
        return false;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (leaderState === "idle") {
      if (e.key === "Escape") {
        e.stopPropagation();
        e.preventDefault();
        enterLeader();
      }
      return;
    }

    if (leaderState === "leader") {
      // Always stop propagation and prevent default in leader mode
      // to prevent xterm or other handlers from receiving the key
      e.stopPropagation();
      e.preventDefault();

      clearLeaderTimeout();
      handleLeaderKey(e);
      resetToIdle();
    }
  }

  onMount(() => {
    window.addEventListener("keydown", onKeydown, { capture: true });
    return () => {
      window.removeEventListener("keydown", onKeydown, { capture: true });
      clearLeaderTimeout();
    };
  });
</script>
