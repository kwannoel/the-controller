import { describe, it, expect } from "vitest";
import { focusAfterSessionDelete, focusAfterProjectDelete, focusForModeSwitch } from "./focus-helpers";
import type { Project, SessionConfig } from "./stores";

function makeSession(id: string): SessionConfig {
  return {
    id,
    label: `session-${id}`,
    worktree_path: null,
    worktree_branch: null,
    archived: false,
    kind: "claude",
    github_issue: null,
    initial_prompt: null,
    auto_worker_session: false,
  };
}

function makeProject(id: string, sessionIds: string[]): Project {
  return {
    id,
    name: `project-${id}`,
    repo_path: `/tmp/${id}`,
    created_at: "2026-01-01",
    archived: false,
    maintainer: { enabled: false, interval_minutes: 60 },
    auto_worker: { enabled: false },
    sessions: sessionIds.map(makeSession),
    prompts: [],
    staged_session: null,
  };
}

describe("focusAfterSessionDelete", () => {
  it("focuses the session above when deleting a non-first session", () => {
    const projects = [makeProject("p1", ["s1", "s2", "s3"])];
    const result = focusAfterSessionDelete(projects, "p1", "s2", false);
    expect(result).toEqual({ type: "session", sessionId: "s1", projectId: "p1" });
  });

  it("focuses the parent project when deleting the first session", () => {
    const projects = [makeProject("p1", ["s1", "s2"])];
    const result = focusAfterSessionDelete(projects, "p1", "s1", false);
    expect(result).toEqual({ type: "project", projectId: "p1" });
  });

  it("focuses the parent project when deleting the only session", () => {
    const projects = [makeProject("p1", ["s1"])];
    const result = focusAfterSessionDelete(projects, "p1", "s1", false);
    expect(result).toEqual({ type: "project", projectId: "p1" });
  });

  it("returns null for unknown project", () => {
    const projects = [makeProject("p1", ["s1"])];
    const result = focusAfterSessionDelete(projects, "unknown", "s1", false);
    expect(result).toBeNull();
  });
});

describe("focusAfterProjectDelete", () => {
  it("focuses last session of project above when it's expanded and has sessions", () => {
    const projects = [
      makeProject("p1", ["s1", "s2"]),
      makeProject("p2", ["s3"]),
    ];
    const expanded = new Set(["p1"]);
    const result = focusAfterProjectDelete(projects, "p2", expanded, false);
    expect(result).toEqual({ type: "session", sessionId: "s2", projectId: "p1" });
  });

  it("focuses the project above when it's collapsed", () => {
    const projects = [
      makeProject("p1", ["s1"]),
      makeProject("p2", ["s3"]),
    ];
    const expanded = new Set<string>();
    const result = focusAfterProjectDelete(projects, "p2", expanded, false);
    expect(result).toEqual({ type: "project", projectId: "p1" });
  });

  it("focuses the project above when it has no visible sessions", () => {
    const projects = [
      makeProject("p1", []),
      makeProject("p2", ["s3"]),
    ];
    const expanded = new Set(["p1"]);
    const result = focusAfterProjectDelete(projects, "p2", expanded, false);
    expect(result).toEqual({ type: "project", projectId: "p1" });
  });

  it("returns null when deleting the topmost project", () => {
    const projects = [makeProject("p1", ["s1"])];
    const expanded = new Set<string>();
    const result = focusAfterProjectDelete(projects, "p1", expanded, false);
    expect(result).toBeNull();
  });

  it("returns null for unknown project", () => {
    const projects = [makeProject("p1", ["s1"])];
    const expanded = new Set<string>();
    const result = focusAfterProjectDelete(projects, "unknown", expanded, false);
    expect(result).toBeNull();
  });
});

describe("focusForModeSwitch", () => {
  it("translates agent focus to active session when switching to development", () => {
    const projects = [makeProject("p1", ["s1", "s2"])];
    const result = focusForModeSwitch(
      { type: "agent", agentKind: "auto-worker", projectId: "p1" },
      "development",
      "s1",
      projects,
    );
    expect(result).toEqual({ type: "session", sessionId: "s1", projectId: "p1" });
  });

  it("translates agent-panel focus to active session when switching to development", () => {
    const projects = [makeProject("p1", ["s1"])];
    const result = focusForModeSwitch(
      { type: "agent-panel", agentKind: "maintainer", projectId: "p1" },
      "development",
      "s1",
      projects,
    );
    expect(result).toEqual({ type: "session", sessionId: "s1", projectId: "p1" });
  });

  it("falls back to project when active session is not in the focused project", () => {
    const projects = [makeProject("p1", ["s1"]), makeProject("p2", ["s2"])];
    const result = focusForModeSwitch(
      { type: "agent", agentKind: "auto-worker", projectId: "p1" },
      "development",
      "s2", // active session is in p2, not p1
      projects,
    );
    expect(result).toEqual({ type: "project", projectId: "p1" });
  });

  it("falls back to project when no active session exists", () => {
    const projects = [makeProject("p1", ["s1"])];
    const result = focusForModeSwitch(
      { type: "agent", agentKind: "auto-worker", projectId: "p1" },
      "development",
      null,
      projects,
    );
    expect(result).toEqual({ type: "project", projectId: "p1" });
  });

  it("translates session focus to project when switching to agents", () => {
    const projects = [makeProject("p1", ["s1"])];
    const result = focusForModeSwitch(
      { type: "session", sessionId: "s1", projectId: "p1" },
      "agents",
      "s1",
      projects,
    );
    expect(result).toEqual({ type: "project", projectId: "p1" });
  });

  it("preserves project focus across mode switches", () => {
    const projects = [makeProject("p1", ["s1"])];
    const focus = { type: "project" as const, projectId: "p1" };
    expect(focusForModeSwitch(focus, "development", "s1", projects)).toBe(focus);
    expect(focusForModeSwitch(focus, "agents", "s1", projects)).toBe(focus);
    expect(focusForModeSwitch(focus, "notes", "s1", projects)).toBe(focus);
  });

  it("returns null when current focus is null", () => {
    expect(focusForModeSwitch(null, "development", null, [])).toBeNull();
    expect(focusForModeSwitch(null, "agents", null, [])).toBeNull();
    expect(focusForModeSwitch(null, "notes", null, [])).toBeNull();
  });

  it("translates session focus to project when switching to notes", () => {
    const projects = [makeProject("p1", ["s1"])];
    const result = focusForModeSwitch(
      { type: "session", sessionId: "s1", projectId: "p1" },
      "notes",
      "s1",
      projects,
    );
    expect(result).toEqual({ type: "project", projectId: "p1" });
  });

  it("translates agent focus to project when switching to notes", () => {
    const projects = [makeProject("p1", ["s1"])];
    const result = focusForModeSwitch(
      { type: "agent", agentKind: "auto-worker", projectId: "p1" },
      "notes",
      "s1",
      projects,
    );
    expect(result).toEqual({ type: "project", projectId: "p1" });
  });

  it("translates note focus to active session when switching to development", () => {
    const projects = [makeProject("p1", ["s1"])];
    const result = focusForModeSwitch(
      { type: "note", filename: "todo.md", projectId: "p1" },
      "development",
      "s1",
      projects,
    );
    expect(result).toEqual({ type: "session", sessionId: "s1", projectId: "p1" });
  });

  it("translates note focus to project when no active session on switch to development", () => {
    const projects = [makeProject("p1", ["s1"])];
    const result = focusForModeSwitch(
      { type: "note", filename: "todo.md", projectId: "p1" },
      "development",
      null,
      projects,
    );
    expect(result).toEqual({ type: "project", projectId: "p1" });
  });

  it("translates notes-editor focus to project when switching to agents", () => {
    const projects = [makeProject("p1", ["s1"])];
    const result = focusForModeSwitch(
      { type: "notes-editor", projectId: "p1" },
      "agents",
      "s1",
      projects,
    );
    expect(result).toEqual({ type: "project", projectId: "p1" });
  });
});
