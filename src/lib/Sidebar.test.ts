import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/svelte";
import { get } from "svelte/store";
import { command, listen } from "$lib/backend";
import { showToast } from "./toast";
import {
  activeSessionId,
  activeNote,
  expandedProjects,
  focusTarget,
  hotkeyAction,
  noteEntries,
  projects,
  selectedSessionProvider,
  sessionStatuses,
  showKeyHints,
  workspaceMode,
} from "./stores";

vi.mock("./toast", () => ({
  showToast: vi.fn(),
}));

vi.mock("./FuzzyFinder.svelte", () => ({
  default: function MockFuzzyFinder() {},
}));
vi.mock("./NewProjectModal.svelte", () => ({
  default: function MockNewProjectModal() {},
}));
vi.mock("./DeleteProjectModal.svelte", () => ({
  default: function MockDeleteProjectModal() {},
}));
vi.mock("./ConfirmModal.svelte", () => ({
  default: function MockConfirmModal() {},
}));
vi.mock("./DeleteSessionModal.svelte", () => ({
  default: function MockDeleteSessionModal() {},
}));
vi.mock("./NewNoteModal.svelte", () => ({
  default: function MockNewNoteModal() {},
}));
vi.mock("./RenameNoteModal.svelte", () => ({
  default: function MockRenameNoteModal() {},
}));
vi.mock("./sidebar/ProjectTree.svelte", () => ({
  default: function MockProjectTree() {},
}));
vi.mock("./sidebar/AgentTree.svelte", () => ({
  default: function MockAgentTree() {},
}));
vi.mock("./sidebar/NotesTree.svelte", () => ({
  default: function MockNotesTree() {},
}));

import Sidebar from "./Sidebar.svelte";

describe("Sidebar provider indicator", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    projects.set([]);
    activeSessionId.set(null);
    sessionStatuses.set(new Map());
    showKeyHints.set(false);
    focusTarget.set(null);
    expandedProjects.set(new Set());
    workspaceMode.set("development");
    activeNote.set(null);
    noteEntries.set(new Map());
    hotkeyAction.set(null);
    selectedSessionProvider.set("claude");

    vi.mocked(command).mockImplementation(async (cmd: string) => {
      if (cmd === "list_projects") return { projects: [], corrupt_entries: [] };
      return;
    });
  });

  it("shows the active provider in the development footer", async () => {
    render(Sidebar);

    await waitFor(() => {
      expect(screen.getByText(/Provider: Claude/i)).toBeInTheDocument();
    });
  });

  it("updates the footer indicator when the selected provider changes", async () => {
    render(Sidebar);

    selectedSessionProvider.set("codex");

    await waitFor(() => {
      expect(screen.getByText(/Provider: Codex/i)).toBeInTheDocument();
    });
  });

  it("surfaces corrupt project metadata returned by list_projects", async () => {
    vi.mocked(command).mockImplementation(async (cmd: string) => {
      if (cmd === "list_projects") {
        return {
          projects: [
            {
              id: "project-1",
              name: "Alpha",
              repo_path: "/tmp/alpha",
              created_at: "2026-03-10T00:00:00Z",
              archived: false,
              sessions: [],
              maintainer: {
                enabled: false,
                interval_minutes: 60,
                github_repo: null,
              },
              auto_worker: { enabled: false },
              prompts: [],
              staged_sessions: [],
            },
          ],
          corrupt_entries: [
            {
              project_dir: "/tmp/.the-controller/projects/bad",
              project_file: "/tmp/.the-controller/projects/bad/project.json",
              error: "expected value at line 1 column 1",
            },
          ],
        };
      }
      return;
    });

    render(Sidebar);

    await waitFor(() => {
      expect(showToast).toHaveBeenCalledWith(
        expect.stringContaining("corrupt project.json"),
        "error",
      );
    });
  });
});

describe("Sidebar session cleanup", () => {
  const sessionId = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
  const projectId = "project-cleanup";
  const projectWithSession = {
    id: projectId,
    name: "Test Project",
    repo_path: "/tmp/test",
    created_at: "2026-03-10",
    archived: false,
    sessions: [
      {
        id: sessionId,
        label: "session-1-abc",
        worktree_path: "/tmp/wt",
        worktree_branch: "session-1-abc",
        archived: false,
        kind: "development",
        github_issue: null,
        initial_prompt: null,
        auto_worker_session: false,
      },
    ],
    maintainer: { enabled: false, interval_minutes: 60, github_repo: null },
    auto_worker: { enabled: false },
    prompts: [],
    staged_sessions: [],
  };

  beforeEach(() => {
    vi.clearAllMocks();
    projects.set([]);
    activeSessionId.set(null);
    sessionStatuses.set(new Map());
    showKeyHints.set(false);
    focusTarget.set(null);
    expandedProjects.set(new Set());
    workspaceMode.set("development");
    activeNote.set(null);
    noteEntries.set(new Map());
    hotkeyAction.set(null);
    selectedSessionProvider.set("claude");
  });

  it("removes the session from the store when session-cleanup event fires", async () => {
    // Capture listen handlers keyed by event name
    const listenHandlers = new Map<string, (payload: unknown) => void>();
    vi.mocked(listen).mockImplementation(((event: string, handler: (payload: unknown) => void) => {
      listenHandlers.set(event, handler);
      return () => {};
    }) as typeof listen);

    // list_projects returns the project (including the session) on first call,
    // and without the session on subsequent calls (backend already cleaned up)
    vi.mocked(command).mockImplementation(async (cmd: string) => {
      if (cmd === "list_projects") {
        return { projects: get(projects).length > 0 ? get(projects) : [projectWithSession], corrupt_entries: [] };
      }
      if (cmd === "list_folders") return [];
      return;
    });

    render(Sidebar);

    // Wait for the initial loadProjects to populate the store
    await waitFor(() => {
      expect(get(projects)).toHaveLength(1);
      expect(get(projects)[0].sessions).toHaveLength(1);
    });

    // Verify the cleanup listener was registered
    const cleanupEvent = `session-cleanup:${sessionId}`;
    expect(listenHandlers.has(cleanupEvent)).toBe(true);

    // Fire the cleanup event — this simulates the backend sending session-cleanup
    listenHandlers.get(cleanupEvent)!("cleanup");

    // The session should be immediately removed from the local store
    const currentProjects = get(projects);
    const project = currentProjects.find(p => p.id === projectId);
    expect(project).toBeDefined();
    expect(project!.sessions.find(s => s.id === sessionId)).toBeUndefined();
  });
});
