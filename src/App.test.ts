import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, waitFor } from "@testing-library/svelte";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  activeSessionId,
  appConfig,
  expandedProjects,
  focusTarget,
  hotkeyAction,
  onboardingComplete,
  projects,
  sessionStatuses,
  showKeyHints,
  sidebarVisible,

} from "./lib/stores";

const mocks = vi.hoisted(() => ({
  openPath: vi.fn(),
  setTitle: vi.fn(),
  unlisten: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ setTitle: mocks.setTitle }),
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
  openPath: mocks.openPath,
}));

vi.mock("./lib/toast", () => ({
  showToast: vi.fn(),
}));

vi.mock("./lib/Sidebar.svelte", () => ({ default: function MockSidebar() {} }));
vi.mock("./lib/TerminalManager.svelte", () => ({ default: function MockTerminalManager() {} }));
vi.mock("./lib/Onboarding.svelte", () => ({ default: function MockOnboarding() {} }));
vi.mock("./lib/Toast.svelte", () => ({ default: function MockToast() {} }));
vi.mock("./lib/HotkeyManager.svelte", () => ({ default: function MockHotkeyManager() {} }));
vi.mock("./lib/HotkeyHelp.svelte", () => ({ default: function MockHotkeyHelp() {} }));

vi.mock("./lib/CreateIssueModal.svelte", () => ({ default: function MockCreateIssueModal() {} }));
vi.mock("./lib/IssuePickerModal.svelte", () => ({ default: function MockIssuePickerModal() {} }));

import App from "./App.svelte";

const baseProject = {
  id: "proj-1",
  name: "project-one",
  repo_path: "/tmp/project-one",
  created_at: "2026-01-01",
  archived: false,
  sessions: [],
};

describe("App screenshot flow", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // @ts-expect-error compile-time constants injected in app builds
    globalThis.__BUILD_COMMIT__ = "test-commit";
    // @ts-expect-error compile-time constants injected in app builds
    globalThis.__BUILD_BRANCH__ = "test-branch";
    // @ts-expect-error compile-time constants injected in app builds
    globalThis.__DEV_PORT__ = "1420";

    projects.set([baseProject]);
    activeSessionId.set(null);
    focusTarget.set({ type: "project", projectId: "proj-1" });
    hotkeyAction.set(null);
    showKeyHints.set(false);
    sidebarVisible.set(true);

    onboardingComplete.set(true);
    appConfig.set({ projects_root: "/tmp/projects" });
    sessionStatuses.set(new Map());
    expandedProjects.set(new Set());
  });

  it("listens for direct idle hook for newly created screenshot session and pastes on idle", async () => {
    let idleHookCallback: ((event: { payload: string }) => void) | null = null;
    vi.mocked(listen).mockImplementation(async (eventName: string, callback: unknown) => {
      if (eventName === "session-status-hook:sess-new") {
        idleHookCallback = callback as (event: { payload: string }) => void;
      }
      return mocks.unlisten;
    });

    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "restore_sessions") return;
      if (cmd === "check_onboarding") return { projects_root: "/tmp/projects" };
      if (cmd === "capture_app_screenshot") return "/tmp/the-controller-screenshot.png";
      if (cmd === "create_session") return "sess-new";
      if (cmd === "list_projects") {
        return [
          {
            ...baseProject,
            sessions: [
              {
                id: "sess-new",
                label: "session-1",
                worktree_path: null,
                worktree_branch: null,
                archived: false,
                kind: "claude",
                github_issue: null,
                initial_prompt: null,
              },
            ],
          },
        ];
      }
      return;
    });

    render(App);
    hotkeyAction.set({ type: "screenshot-to-session" });

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("capture_app_screenshot");
      expect(invoke).toHaveBeenCalledWith("create_session", { projectId: "proj-1", kind: "claude" });
    });

    await waitFor(() => {
      expect(listen).toHaveBeenCalledWith("session-status-hook:sess-new", expect.any(Function));
    });

    expect(idleHookCallback).not.toBeNull();
    idleHookCallback!({ payload: "idle" });

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("write_to_pty", {
        sessionId: "sess-new",
        data: "\x1b[200~\x1b[201~",
      });
    });
    expect(mocks.unlisten).toHaveBeenCalledTimes(1);
  });
});
