import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/svelte";
import { get } from "svelte/store";
import {
  projects,
  hotkeyAction,
  focusTarget,
  sidebarVisible,
  expandedProjects,
  workspaceMode,
  workspaceModePickerVisible,
  type Project,
  type SessionConfig,
} from "./stores";
import HotkeyManager from "./HotkeyManager.svelte";
import { daemonStore } from "./daemon/store.svelte";
import type { DaemonSession } from "./daemon/types";

vi.mock("./toast", () => ({
  showToast: vi.fn(),
}));

function makeSession(id: string, label: string, kind = "claude"): SessionConfig {
  return {
    id,
    label,
    worktree_path: null,
    worktree_branch: null,
    archived: false,
    kind,
    github_issue: null,
    initial_prompt: null,
    auto_worker_session: false,
  };
}

function makeProject(id: string, name: string, repoPath: string, sessions: SessionConfig[] = []): Project {
  return {
    id,
    name,
    repo_path: repoPath,
    created_at: "2026-01-01",
    archived: false,
    maintainer: { enabled: false, interval_minutes: 60 },
    auto_worker: { enabled: false },
    sessions,
    prompts: [],
    staged_sessions: [],
  };
}

const testProject = makeProject("proj-1", "test-project", "/tmp/test", [
  makeSession("sess-1", "session-1"),
  makeSession("sess-2", "session-2"),
]);

const testProject2 = makeProject("proj-2", "other-project", "/tmp/other", [
  makeSession("sess-3", "session-1"),
]);

function makeDaemonSession(id: string, cwd: string, label = id): DaemonSession {
  return {
    id,
    label,
    agent: "claude",
    cwd,
    args: [],
    status: "running",
    native_session_id: null,
    pid: null,
    created_at: 0,
    updated_at: 0,
    ended_at: null,
    end_reason: null,
  };
}

function pressKey(key: string) {
  window.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true }));
}

function pressMetaKey(key: string) {
  window.dispatchEvent(new KeyboardEvent("keydown", { key, metaKey: true, bubbles: true }));
}

describe("HotkeyManager", () => {
  beforeEach(() => {
    projects.set([testProject, testProject2]);
    hotkeyAction.set(null);
    focusTarget.set(null);
    sidebarVisible.set(true);
    expandedProjects.set(new Set(["proj-1", "proj-2"]));
    workspaceMode.set("chat");
    workspaceModePickerVisible.set(false);
    daemonStore.sessions.clear();
    daemonStore.activeSessionId = null;
    vi.clearAllMocks();
    render(HotkeyManager);
  });

  afterEach(() => {
    cleanup();
    daemonStore.sessions.clear();
    daemonStore.activeSessionId = null;
  });

  it("f dispatches open-fuzzy-finder action", () => {
    let captured: unknown = null;
    const unsub = hotkeyAction.subscribe((v) => { captured = v; });
    pressKey("f");
    expect(captured).toEqual({ type: "open-fuzzy-finder" });
    unsub();
  });

  it("? dispatches toggle-help action", () => {
    let captured: unknown = null;
    const unsub = hotkeyAction.subscribe((v) => { captured = v; });
    pressKey("?");
    expect(captured).toEqual({ type: "toggle-help" });
    unsub();
  });

  it("removed session-management keys do not dispatch in chat mode", () => {
    const devKeys = ["n", "d", "m", "v", "p", "P", "y"];
    for (const key of devKeys) pressKey(key);
    expect(get(hotkeyAction)).toBeNull();
  });

  it("Cmd+T no longer toggles a session provider", () => {
    pressMetaKey("t");
    expect(get(hotkeyAction)).toBeNull();
  });

  it("Space opens the workspace mode picker", () => {
    pressKey(" ");
    expect(get(workspaceModePickerVisible)).toBe(true);
  });

  it("Space switches only among agents, kanban, and chat", () => {
    pressKey(" ");
    pressKey("a");
    expect(get(workspaceMode)).toBe("agents");

    pressKey(" ");
    pressKey("k");
    expect(get(workspaceMode)).toBe("kanban");

    pressKey(" ");
    pressKey("c");
    expect(get(workspaceMode)).toBe("chat");
  });

  it("Space then d closes the picker without leaving the current mode", () => {
    workspaceMode.set("agents");
    pressKey(" ");
    pressKey("d");
    expect(get(workspaceMode)).toBe("agents");
    expect(get(workspaceModePickerVisible)).toBe(false);
  });

  it("Escape closes picker without changing mode", () => {
    pressKey(" ");
    pressKey("Escape");
    expect(get(workspaceMode)).toBe("chat");
    expect(get(workspaceModePickerVisible)).toBe(false);
  });

  it("ignores hotkeys while an input is focused", () => {
    const input = document.createElement("input");
    document.body.appendChild(input);
    input.focus();

    pressKey("f");
    expect(get(hotkeyAction)).toBeNull();

    input.remove();
  });

  it("j/k walks project rows in kanban mode without PTY sessions", () => {
    workspaceMode.set("kanban");
    focusTarget.set({ type: "project", projectId: "proj-1" });

    pressKey("j");
    expect(get(focusTarget)).toEqual({ type: "project", projectId: "proj-2" });

    pressKey("k");
    expect(get(focusTarget)).toEqual({ type: "project", projectId: "proj-1" });
  });

  it("agent keys work only in agents mode", () => {
    workspaceMode.set("agents");
    focusTarget.set({ type: "agent", agentKind: "auto-worker", projectId: "proj-1" });

    let captured: unknown = null;
    const unsub = hotkeyAction.subscribe((v) => { captured = v; });
    pressKey("o");
    expect(captured).toEqual({ type: "toggle-auto-worker-enabled" });

    captured = null;
    pressKey("r");
    expect(captured).toEqual({ type: "trigger-maintainer-check" });

    captured = null;
    pressKey("c");
    expect(captured).toEqual({ type: "clear-maintainer-reports" });
    unsub();
  });

  it("l on an agent focuses the agent panel", () => {
    workspaceMode.set("agents");
    focusTarget.set({ type: "agent", agentKind: "maintainer", projectId: "proj-1" });
    pressKey("l");
    expect(get(focusTarget)).toEqual({ type: "agent-panel", agentKind: "maintainer", projectId: "proj-1" });
  });

  it("chat navigation walks daemon sessions and skips PTY sessions", () => {
    daemonStore.sessions.set("c1", makeDaemonSession("c1", "/tmp/test", "chat-1"));
    daemonStore.sessions.set("c2", makeDaemonSession("c2", "/tmp/test", "chat-2"));
    daemonStore.sessions.set("c3", makeDaemonSession("c3", "/tmp/other", "chat-3"));

    focusTarget.set({ type: "project", projectId: "proj-1" });
    pressKey("j");
    expect(get(focusTarget)).toEqual({ type: "session", sessionId: "c1", projectId: "proj-1" });
    expect(daemonStore.activeSessionId).toBe("c1");

    pressKey("j");
    expect(get(focusTarget)).toEqual({ type: "session", sessionId: "c2", projectId: "proj-1" });

    pressKey("j");
    expect(get(focusTarget)).toEqual({ type: "project", projectId: "proj-2" });
  });

  it("Enter on a chat session selects it", () => {
    daemonStore.sessions.set("c1", makeDaemonSession("c1", "/tmp/test", "chat-1"));
    focusTarget.set({ type: "session", sessionId: "c1", projectId: "proj-1" });

    pressKey("Enter");
    expect(daemonStore.activeSessionId).toBe("c1");
  });

  it("Escape moves session and agent focus back to project", () => {
    focusTarget.set({ type: "session", sessionId: "c1", projectId: "proj-1" });
    pressKey("Escape");
    expect(get(focusTarget)).toEqual({ type: "project", projectId: "proj-1" });

    focusTarget.set({ type: "agent", agentKind: "maintainer", projectId: "proj-1" });
    pressKey("Escape");
    expect(get(focusTarget)).toEqual({ type: "project", projectId: "proj-1" });
  });
});
