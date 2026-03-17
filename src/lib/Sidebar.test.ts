import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/svelte";
import { command } from "$lib/backend";
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
