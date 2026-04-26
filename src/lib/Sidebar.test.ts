import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/svelte";
import { command } from "$lib/backend";
import { showToast } from "./toast";
import {
  expandedProjects,
  focusTarget,
  hotkeyAction,
  projects,
  showKeyHints,
  workspaceMode,
} from "./stores";

vi.mock("./toast", () => ({
  showToast: vi.fn(),
}));

vi.mock("./sidebar/AgentTree.svelte", () => ({
  default: function MockAgentTree() {},
}));
vi.mock("./sidebar/ProjectList.svelte", () => ({
  default: function MockProjectList() {},
}));
vi.mock("./chat/ChatSessionList.svelte", () => ({
  default: function MockChatSessionList() {},
}));

import Sidebar from "./Sidebar.svelte";

describe("Sidebar", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    projects.set([]);
    showKeyHints.set(false);
    focusTarget.set(null);
    expandedProjects.set(new Set());
    workspaceMode.set("chat");
    hotkeyAction.set(null);

    vi.mocked(command).mockImplementation(async (cmd: string) => {
      if (cmd === "list_projects") return { projects: [], corrupt_entries: [] };
      return;
    });
  });

  it("shows Chat in the default sidebar header", async () => {
    render(Sidebar);

    await waitFor(() => {
      expect(screen.getByText("Chat")).toBeInTheDocument();
    });
  });

  it("updates the sidebar header for agents and kanban", async () => {
    render(Sidebar);

    workspaceMode.set("agents");
    await waitFor(() => {
      expect(screen.getByText("Agents")).toBeInTheDocument();
    });

    workspaceMode.set("kanban");
    await waitFor(() => {
      expect(screen.getByText("Kanban")).toBeInTheDocument();
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
