import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/svelte";
import { invoke } from "@tauri-apps/api/core";
import {
  activeSessionId,
  activeNote,
  archiveView,
  archivedProjects,
  expandedProjects,
  focusTarget,
  hotkeyAction,
  jumpMode,
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

vi.mock("./FuzzyFinder.svelte", () => ({ default: function MockFuzzyFinder() {} }));
vi.mock("./NewProjectModal.svelte", () => ({ default: function MockNewProjectModal() {} }));
vi.mock("./DeleteProjectModal.svelte", () => ({ default: function MockDeleteProjectModal() {} }));
vi.mock("./ConfirmModal.svelte", () => ({ default: function MockConfirmModal() {} }));
vi.mock("./DeleteSessionModal.svelte", () => ({ default: function MockDeleteSessionModal() {} }));
vi.mock("./NewNoteModal.svelte", () => ({ default: function MockNewNoteModal() {} }));
vi.mock("./RenameNoteModal.svelte", () => ({ default: function MockRenameNoteModal() {} }));
vi.mock("./sidebar/ProjectTree.svelte", () => ({ default: function MockProjectTree() {} }));
vi.mock("./sidebar/AgentTree.svelte", () => ({ default: function MockAgentTree() {} }));
vi.mock("./sidebar/NotesTree.svelte", () => ({ default: function MockNotesTree() {} }));

import Sidebar from "./Sidebar.svelte";

describe("Sidebar provider indicator", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    projects.set([]);
    activeSessionId.set(null);
    sessionStatuses.set(new Map());
    showKeyHints.set(false);
    jumpMode.set(null);
    archiveView.set(false);
    archivedProjects.set([]);
    focusTarget.set(null);
    expandedProjects.set(new Set());
    workspaceMode.set("development");
    activeNote.set(null);
    noteEntries.set(new Map());
    hotkeyAction.set(null);
    selectedSessionProvider.set("claude");

    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "list_projects") return [];
      if (cmd === "list_archived_projects") return [];
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
});
