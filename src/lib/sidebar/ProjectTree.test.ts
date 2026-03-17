import { describe, it, expect, vi, beforeEach, type Mock } from "vitest";
import { render } from "@testing-library/svelte";
import type { Project, FocusTarget, SessionStatus, SessionConfig } from "../stores";

import ProjectTree from "./ProjectTree.svelte";

function makeSession(overrides: Partial<SessionConfig>): SessionConfig {
  return {
    id: "sess-default",
    label: "session-default",
    worktree_path: null,
    worktree_branch: null,
    archived: false,
    kind: "claude",
    github_issue: null,
    initial_prompt: null,
    auto_worker_session: false,
    ...overrides,
  };
}

const baseProjects: Project[] = [
  {
    id: "proj-1",
    name: "Project One",
    repo_path: "/tmp/proj-1",
    created_at: "2026-01-01",
    archived: false,
    maintainer: { enabled: false, interval_minutes: 60 },
    auto_worker: { enabled: false },
    prompts: [],
    staged_sessions: [],
    sessions: [
      makeSession({
        id: "sess-1",
        label: "session-1",
        github_issue: { number: 42, title: "Issue", url: "https://example.com", labels: [] },
      }),
      makeSession({
        id: "sess-2",
        label: "session-2",
        github_issue: null,
      }),
    ],
  },
];

describe("ProjectTree", () => {
  let onToggleProjectSpy: Mock<(projectId: string) => void>;
  let onProjectFocusSpy: Mock<(projectId: string) => void>;
  let onSessionFocusSpy: Mock<(sessionId: string, projectId: string) => void>;
  let onSessionSelectSpy: Mock<(sessionId: string, projectId: string) => void>;
  let onToggleProject: (projectId: string) => void;
  let onProjectFocus: (projectId: string) => void;
  let onSessionFocus: (sessionId: string, projectId: string) => void;
  let onSessionSelect: (sessionId: string, projectId: string) => void;

  beforeEach(() => {
    onToggleProjectSpy = vi.fn<(projectId: string) => void>();
    onProjectFocusSpy = vi.fn<(projectId: string) => void>();
    onSessionFocusSpy = vi.fn<(sessionId: string, projectId: string) => void>();
    onSessionSelectSpy = vi.fn<(sessionId: string, projectId: string) => void>();
    onToggleProject = (projectId: string) => onToggleProjectSpy(projectId);
    onProjectFocus = (projectId: string) => onProjectFocusSpy(projectId);
    onSessionFocus = (sessionId: string, projectId: string) => onSessionFocusSpy(sessionId, projectId);
    onSessionSelect = (sessionId: string, projectId: string) => onSessionSelectSpy(sessionId, projectId);
  });

  function renderTree(focus: FocusTarget = null, statuses = new Map<string, SessionStatus>()) {
    return render(ProjectTree, {
      projects: baseProjects,
      expandedProjectSet: new Set(["proj-1"]),
      activeSession: "sess-1",
      currentFocus: focus,
      getSessionStatus: (id: string) => statuses.get(id) ?? "idle",
      onToggleProject,
      onProjectFocus,
      onSessionFocus,
      onSessionSelect,
    });
  }

  it("renders active-mode session rows with issue badge and status classes", () => {
    const { container, getByText, queryByText } = renderTree(null, new Map([["sess-1", "working"]]));

    expect(getByText("Project One")).toBeTruthy();
    expect(getByText("session-1")).toBeTruthy();
    expect(getByText("#42")).toBeTruthy();
    expect(getByText("session-2")).toBeTruthy();

    const row = container.querySelector('[data-session-id="sess-1"]');
    expect(row?.classList.contains("active")).toBe(true);

    const dot = row?.querySelector(".status-dot");
    expect(dot?.classList.contains("working")).toBe(true);
  });

  it("calls session callbacks on active row click", async () => {
    const { container } = renderTree();
    const row = container.querySelector('[data-session-id="sess-1"]') as HTMLElement;

    row.click();

    expect(onSessionSelectSpy).toHaveBeenCalledWith("sess-1", "proj-1");
    expect(onSessionFocusSpy).toHaveBeenCalledWith("sess-1", "proj-1");
  });

  it("renders all visible sessions as interactive rows", () => {
    const { container } = renderTree();

    const row = container.querySelector('[data-session-id="sess-2"]') as HTMLElement;
    expect(row.getAttribute("role")).toBe("button");
    expect(row.classList.contains("archived")).toBe(false);
  });
});
