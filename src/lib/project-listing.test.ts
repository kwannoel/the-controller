import { beforeEach, describe, expect, it, vi } from "vitest";
import { command } from "$lib/backend";
import { get } from "svelte/store";
import { projects } from "./stores";
import { refreshProjectsFromBackend } from "./project-listing";

vi.mock("$lib/backend", () => ({
  command: vi.fn(),
  listen: vi.fn(() => () => {}),
}));

describe("refreshProjectsFromBackend", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    projects.set([]);
  });

  it("applies projects and returns corrupt entries from the list_projects response", async () => {
    vi.mocked(command).mockResolvedValue({
      projects: [
        {
          id: "proj-1",
          name: "test-project",
          repo_path: "/tmp/test-project",
          created_at: "2026-01-01",
          archived: false,
          maintainer: { enabled: false, interval_minutes: 60 },
          auto_worker: { enabled: false },
          sessions: [],
          prompts: [],
          staged_sessions: [],
        },
      ],
      corrupt_entries: [
        {
          project_dir: "/tmp/.the-controller/projects/bad",
          project_file: "/tmp/.the-controller/projects/bad/project.json",
          error: "expected ident at line 1 column 3",
        },
      ],
    });

    const result = await refreshProjectsFromBackend();

    expect(command).toHaveBeenCalledWith("list_projects");
    expect(get(projects)).toHaveLength(1);
    expect(get(projects)[0].name).toBe("test-project");
    expect(result.corrupt_entries).toEqual([
      {
        project_dir: "/tmp/.the-controller/projects/bad",
        project_file: "/tmp/.the-controller/projects/bad/project.json",
        error: "expected ident at line 1 column 3",
      },
    ]);
  });
});
