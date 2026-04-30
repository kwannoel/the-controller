<script lang="ts">
  import { onMount } from "svelte";
  import { fromStore } from "svelte/store";
  import {
    projects,
    workspaceMode,
    workspaceModePickerVisible,
    focusTarget,
    expandedProjects,
    dispatchHotkeyAction,
    type Project,
    type HotkeyAction,
    type FocusTarget,
  } from "./stores";
  import { toggleKeystrokeVisualizer, pushKeystroke } from "./keystroke-visualizer";
  import { buildKeyMap, type CommandId } from "./commands";
  import { focusForModeSwitch } from "./focus-helpers";
  import { daemonStore } from "./daemon/store.svelte";

  let workspaceModeActive = $state(false);

  const projectsState = fromStore(projects);
  let projectList: Project[] = $derived(projectsState.current);
  const focusTargetState = fromStore(focusTarget);
  let currentFocus: FocusTarget = $derived(focusTargetState.current);
  const expandedProjectsState = fromStore(expandedProjects);
  let expandedSet: Set<string> = $derived(expandedProjectsState.current);
  const workspaceModeState = fromStore(workspaceMode);
  let currentMode = $derived(workspaceModeState.current);
  let keyMap = $derived(buildKeyMap(currentMode));

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

  function dispatchAction(action: NonNullable<HotkeyAction>) {
    dispatchHotkeyAction(action);
  }

  function projectForChatAction(): Project | undefined {
    let projectId = currentFocus && "projectId" in currentFocus ? currentFocus.projectId : null;
    if (!projectId && daemonStore.activeSessionId) {
      const activeSession = daemonStore.sessions.get(daemonStore.activeSessionId);
      const activeProject = activeSession
        ? projectList.find((project) => project.repo_path === activeSession.cwd)
        : undefined;
      projectId = activeProject?.id ?? null;
    }
    if (!projectId && daemonStore.activeChatId) {
      projectId = daemonStore.chats.get(daemonStore.activeChatId)?.project_id ?? null;
    }
    return projectList.find((project) => project.id === projectId) ?? projectList[0];
  }

  function openNewChatForFocusedProject() {
    const project = projectForChatAction();
    if (!project) return;
    daemonStore.newChatTarget = { projectId: project.id, projectCwd: project.repo_path };
  }

  function switchWorkspaceMode(mode: typeof currentMode) {
    workspaceMode.set(mode);
    const newFocus = focusForModeSwitch(currentFocus, mode);
    if (newFocus !== currentFocus) focusTarget.set(newFocus);
  }

  function handleWorkspaceModeKey(key: string) {
    workspaceModeActive = false;
    workspaceModePickerVisible.set(false);
    if (key === "a") {
      switchWorkspaceMode("agents");
      return;
    }
    if (key === "k") {
      switchWorkspaceMode("kanban");
      return;
    }
    if (key === "c") {
      switchWorkspaceMode("chat");
      return;
    }
    if (key === "p") {
      switchWorkspaceMode("agent-create");
      return;
    }
    if (key === "o") {
      switchWorkspaceMode("agent-observe");
    }
  }

  type SidebarItem =
    | { type: "project"; projectId: string }
    | { type: "session"; sessionId: string; projectId: string }
    | { type: "chat"; chatId: string; projectId: string }
    | { type: "agent"; agentKind: "auto-worker" | "maintainer"; projectId: string };

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

    if (currentMode === "chat") {
      const result: SidebarItem[] = [];
      const daemonSessions = [...daemonStore.sessions.values()];
      const daemonChats = [...daemonStore.chats.values()];
      for (const p of projectList) {
        result.push({ type: "project", projectId: p.id });
        if (!expandedSet.has(p.id)) continue;
        for (const s of daemonSessions) {
          if (s.cwd === p.repo_path) {
            result.push({ type: "session", sessionId: s.id, projectId: p.id });
          }
        }
        for (const chat of daemonChats) {
          if (chat.project_id === p.id) {
            result.push({ type: "chat", chatId: chat.id, projectId: p.id });
          }
        }
      }
      return result;
    }

    return projectList.map((p) => ({ type: "project", projectId: p.id }));
  }

  function navigateItem(direction: 1 | -1) {
    const items = getVisibleItems();
    if (items.length === 0) return;
    let idx = -1;
    if (currentFocus?.type === "session") {
      idx = items.findIndex(it => it.type === "session" && it.sessionId === currentFocus.sessionId);
    } else if (currentFocus?.type === "chat") {
      idx = items.findIndex(it => it.type === "chat" && it.chatId === currentFocus.chatId);
    } else if (currentFocus?.type === "agent") {
      idx = items.findIndex(it => it.type === "agent" && it.projectId === currentFocus.projectId && it.agentKind === currentFocus.agentKind);
    } else if (currentFocus?.type === "project") {
      idx = items.findIndex(it => it.type === "project" && it.projectId === currentFocus.projectId);
    }
    const len = items.length;
    const next = items[((idx + direction) % len + len) % len];
    if (next.type === "session") {
      daemonStore.activeSessionId = next.sessionId;
      daemonStore.activeChatId = null;
      focusTarget.set({ type: "session", sessionId: next.sessionId, projectId: next.projectId });
    } else if (next.type === "chat") {
      daemonStore.activeChatId = next.chatId;
      daemonStore.activeSessionId = null;
      focusTarget.set({ type: "chat", chatId: next.chatId, projectId: next.projectId });
    } else if (next.type === "agent") {
      focusTarget.set({ type: "agent", agentKind: next.agentKind, projectId: next.projectId });
    } else {
      focusTarget.set({ type: "project", projectId: next.projectId });
    }
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
          daemonStore.activeSessionId = currentFocus.sessionId;
          daemonStore.activeChatId = null;
        } else if (currentFocus?.type === "chat") {
          daemonStore.activeChatId = currentFocus.chatId;
          daemonStore.activeSessionId = null;
        } else if (currentFocus?.type === "agent") {
          focusTarget.set({ type: "agent-panel", agentKind: currentFocus.agentKind, projectId: currentFocus.projectId });
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
      case "toggle-maintainer-view":
        dispatchAction({ type: "toggle-maintainer-view" });
        return true;
      case "new-chat":
        openNewChatForFocusedProject();
        return true;
      case "focus-chat-input":
        dispatchAction({ type: "focus-chat-input" });
        return true;
      default: {
        const _exhaustive: never = id;
        return false;
      }
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (["Shift", "Control", "Alt", "Meta"].includes(e.key)) return;
    if (e.repeat) return;

    if (e.metaKey && e.key === "k") {
      e.stopPropagation();
      e.preventDefault();
      toggleKeystrokeVisualizer();
      return;
    }

    if (workspaceModeActive) {
      e.stopPropagation();
      e.preventDefault();
      handleWorkspaceModeKey(e.key);
      pushKeystroke("␣" + e.key);
      return;
    }

    if (isDialogOpen()) return;
    if (isEditableElementFocused()) return;

    if (e.key === "Escape") {
      if (currentFocus?.type === "session" || currentFocus?.type === "chat") {
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

    if (e.key === " ") {
      e.stopPropagation();
      e.preventDefault();
      workspaceModeActive = true;
      workspaceModePickerVisible.set(true);
      pushKeystroke("␣");
      return;
    }

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

    if (handleHotkey(e.key)) {
      e.stopPropagation();
      e.preventDefault();
      pushKeystroke(e.key);
    }
  }

  onMount(() => {
    window.addEventListener("keydown", onKeydown, { capture: true });
    return () => {
      window.removeEventListener("keydown", onKeydown, { capture: true });
    };
  });
</script>
