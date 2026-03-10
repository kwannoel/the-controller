import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import AgentDashboard from "./AgentDashboard.svelte";
import {
  autoWorkerStatuses,
  focusTarget,
  hotkeyAction,
  maintainerErrors,
  maintainerStatuses,
  projects,
  type Project,
  type WorkerReport,
} from "./stores";

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: vi.fn(),
}));

vi.mock("./toast", () => ({
  showToast: vi.fn(),
}));

function makeProject(): Project {
  return {
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
    auto_worker: { enabled: true },
    prompts: [],
    staged_session: null,
  };
}

describe("AgentDashboard auto-worker pane", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    projects.set([makeProject()]);
    focusTarget.set({ type: "agent", agentKind: "auto-worker", projectId: "project-1" });
    autoWorkerStatuses.set(new Map([["project-1", { status: "idle" }]]));
    maintainerStatuses.set(new Map());
    maintainerErrors.set(new Map());
    hotkeyAction.set(null);
    vi.mocked(openUrl).mockResolvedValue();
  });

  it("shows completed worker issues and the assigned-to-auto-worker policy tag", async () => {
    const reports: WorkerReport[] = [
      {
        issue_number: 299,
        title: "Sanitize markdown link URLs",
        comment_body: "No worker report was posted for this issue.",
        updated_at: "2026-03-10T07:04:46Z",
      },
    ];

    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_worker_reports") return reports;
      return [];
    });

    render(AgentDashboard);

    await waitFor(() => {
      expect(screen.getByText("#299 Sanitize markdown link URLs")).toBeInTheDocument();
    });

    expect(screen.getByText("assigned-to-auto-worker")).toBeInTheDocument();
    expect(screen.queryByText("finished-by-worker")).not.toBeInTheDocument();
  });

  it("shows fallback report text when a completed issue has no worker report comment", async () => {
    const reports: WorkerReport[] = [
      {
        issue_number: 299,
        title: "Sanitize markdown link URLs",
        comment_body: "No worker report was posted for this issue.",
        updated_at: "2026-03-10T07:04:46Z",
      },
    ];

    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_worker_reports") return reports;
      return [];
    });

    render(AgentDashboard);

    const report = await screen.findByText("#299 Sanitize markdown link URLs");
    await fireEvent.click(report);

    await waitFor(() => {
      expect(screen.getByText("No worker report was posted for this issue.")).toBeInTheDocument();
    });
  });
});
