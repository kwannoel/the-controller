import { beforeEach, describe, expect, it, vi, afterEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { command } from "$lib/backend";
import TriagePanel from "./TriagePanel.svelte";
import { focusTarget, projects, type Project } from "./stores";

const baseProject: Project = {
  id: "project-1",
  name: "Controller",
  repo_path: "/tmp/controller",
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
  staged_session: null,
};

describe("TriagePanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    projects.set([baseProject]);
    focusTarget.set({ type: "project", projectId: "project-1" });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("writes the canonical complexity:low label for low-complexity triage", async () => {
    vi.mocked(command).mockImplementation(async (cmd: string) => {
      if (cmd === "list_github_issues") {
        return [
          {
            number: 42,
            title: "Normalize complexity labels",
            url: "https://example.com/issues/42",
            body: "Body",
            labels: [],
          },
        ];
      }

      return undefined;
    });

    render(TriagePanel, {
      props: {
        category: "untriaged",
        onClose: vi.fn(),
      },
    });

    await screen.findByText("Normalize complexity labels");

    await fireEvent.click(screen.getByText("Low priority"));
    await fireEvent.click(await screen.findByText("Low complexity"));
    await vi.advanceTimersByTimeAsync(300);

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("add_github_label", expect.objectContaining({
        issueNumber: 42,
        label: "complexity:low",
      }));
    });
  });
});
