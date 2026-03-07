import { describe, it, expect, vi, beforeEach } from "vitest";
import { render } from "@testing-library/svelte";
import type { Project, FocusTarget, JumpPhase, SessionStatus } from "../stores";

import ProjectTree from "./ProjectTree.svelte";

const baseProjects: Project[] = [
  {
    id: "proj-1",
    name: "Project One",
    repo_path: "/tmp/proj-1",
    created_at: "2026-01-01",
    archived: false,
    maintainer: { enabled: false, interval_minutes: 60 },
    sessions: [
      {
        id: "sess-1",
        label: "session-1",
        worktree_path: null,
        worktree_branch: null,
        archived: false,
        kind: "claude",
        github_issue: { number: 42, title: "Issue", url: "https://example.com", labels: [] },
        initial_prompt: null,
      },
      {
        id: "sess-2",
        label: "session-2",
        worktree_path: null,
        worktree_branch: null,
        archived: true,
        kind: "claude",
        github_issue: null,
        initial_prompt: null,
      },
    ],
  },
];

describe("ProjectTree", () => {
  let onToggleProjectSpy: ReturnType<typeof vi.fn>;
  let onProjectFocusSpy: ReturnType<typeof vi.fn>;
  let onSessionFocusSpy: ReturnType<typeof vi.fn>;
  let onSessionSelectSpy: ReturnType<typeof vi.fn>;
  let onToggleProject: (projectId: string) => void;
  let onProjectFocus: (projectId: string) => void;
  let onSessionFocus: (sessionId: string, projectId: string) => void;
  let onSessionSelect: (sessionId: string, projectId: string) => void;

  beforeEach(() => {
    onToggleProjectSpy = vi.fn();
    onProjectFocusSpy = vi.fn();
    onSessionFocusSpy = vi.fn();
    onSessionSelectSpy = vi.fn();
    onToggleProject = (projectId: string) => onToggleProjectSpy(projectId);
    onProjectFocus = (projectId: string) => onProjectFocusSpy(projectId);
    onSessionFocus = (sessionId: string, projectId: string) => onSessionFocusSpy(sessionId, projectId);
    onSessionSelect = (sessionId: string, projectId: string) => onSessionSelectSpy(sessionId, projectId);
  });

  function renderTree(mode: "active" | "archived", focus: FocusTarget = null, statuses = new Map<string, SessionStatus>()) {
    return render(ProjectTree, {
      projects: baseProjects,
      mode,
      expandedProjectSet: new Set(["proj-1"]),
      activeSession: "sess-1",
      currentFocus: focus,
      jumpState: { phase: "project" } as JumpPhase,
      projectJumpLabels: ["z"],
      getSessionStatus: (id: string) => statuses.get(id) ?? "idle",
      onToggleProject,
      onProjectFocus,
      onSessionFocus,
      onSessionSelect,
    });
  }

  it("renders active-mode session rows with issue badge and status classes", () => {
    const { container, getByText, queryByText } = renderTree("active", null, new Map([["sess-1", "working"]]));

    expect(getByText("Project One")).toBeTruthy();
    expect(getByText("session-1")).toBeTruthy();
    expect(getByText("#42")).toBeTruthy();
    expect(queryByText("session-2")).toBeNull();

    const row = container.querySelector('[data-session-id="sess-1"]');
    expect(row?.classList.contains("active")).toBe(true);

    const dot = row?.querySelector(".status-dot");
    expect(dot?.classList.contains("working")).toBe(true);
  });

  it("calls session callbacks on active row click", async () => {
    const { container } = renderTree("active");
    const row = container.querySelector('[data-session-id="sess-1"]') as HTMLElement;

    row.click();

    expect(onSessionSelectSpy).toHaveBeenCalledWith("sess-1", "proj-1");
    expect(onSessionFocusSpy).toHaveBeenCalledWith("sess-1", "proj-1");
  });

  it("renders archived sessions as non-interactive rows", () => {
    const { container, getByText, queryByText } = renderTree("archived");

    expect(getByText("session-2")).toBeTruthy();
    expect(queryByText("session-1")).toBeNull();

    const row = container.querySelector('[data-session-id="sess-2"]') as HTMLElement;
    expect(row.getAttribute("role")).toBeNull();
    expect(row.classList.contains("archived")).toBe(true);
  });
});
