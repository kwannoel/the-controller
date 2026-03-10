import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { command } from "$lib/backend";
import {
  activeSessionId,
  appConfig,
  expandedProjects,
  focusTarget,
  hotkeyAction,
  onboardingComplete,
  projects,
  selectedSessionProvider,
  sessionStatuses,
  showKeyHints,
  sidebarVisible,
  type Project,
} from "./lib/stores";

const mocks = vi.hoisted(() => ({
  openPath: vi.fn(),
  setTitle: vi.fn(),
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

vi.mock("./lib/CreateIssueModal.svelte", async () => ({
  default: (await import("./test/CreateIssueModalMock.svelte")).default,
}));
vi.mock("./lib/IssuePickerModal.svelte", async () => ({
  default: (await import("./test/IssuePickerModalMock.svelte")).default,
}));

import App from "./App.svelte";

const baseProject: Project = {
  id: "proj-1",
  name: "the-controller",
  repo_path: "/tmp/the-controller",
  created_at: "2026-01-01",
  archived: false,
  maintainer: { enabled: false, interval_minutes: 60 },
  auto_worker: { enabled: false },
  sessions: [],
  prompts: [],
  staged_session: null,
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
    selectedSessionProvider.set("claude");

    onboardingComplete.set(true);
    appConfig.set({ projects_root: "/tmp/projects" });
    sessionStatuses.set(new Map());
    expandedProjects.set(new Set());
  });

  function setupMocks() {
    vi.mocked(command).mockImplementation(async (cmd: string) => {
      if (cmd === "restore_sessions") return;
      if (cmd === "check_onboarding") return { projects_root: "/tmp/projects" };
      if (cmd === "capture_app_screenshot") return "/tmp/the-controller-screenshot.png";
      if (cmd === "create_session") return "sess-new";
      if (cmd === "list_projects") {
        return {
          projects: [
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
                  auto_worker_session: false,
                },
              ],
            },
          ],
          corrupt_entries: [],
        };
      }
      return;
    });
  }

  it("Cmd+S: captures screenshot without preview", async () => {
    setupMocks();
    render(App);
    hotkeyAction.set({ type: "screenshot-to-session" });

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("capture_app_screenshot", { cropped: false });
      expect(command).toHaveBeenCalledWith("create_session", expect.objectContaining({
        projectId: "proj-1",
        kind: "claude",
        initialPrompt: expect.stringContaining("/tmp/the-controller-screenshot.png"),
      }));
    });

    expect(mocks.openPath).not.toHaveBeenCalled();
  });

  it("uses the focused project for screenshot sessions even when the project name differs", async () => {
    setupMocks();
    projects.set([{ ...baseProject, name: "client-app", repo_path: "/tmp/client-app" }]);
    focusTarget.set({ type: "project", projectId: "proj-1" });

    render(App);
    hotkeyAction.set({ type: "screenshot-to-session" });

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("create_session", expect.objectContaining({
        projectId: "proj-1",
        kind: "claude",
        initialPrompt: expect.stringContaining("/tmp/the-controller-screenshot.png"),
      }));
    });
  });

  it("uses the selected provider for screenshot sessions", async () => {
    setupMocks();
    selectedSessionProvider.set("codex");

    render(App);
    hotkeyAction.set({ type: "screenshot-to-session" });

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("create_session", expect.objectContaining({
        projectId: "proj-1",
        kind: "codex",
        initialPrompt: expect.stringContaining("/tmp/the-controller-screenshot.png"),
      }));
    });
  });

  it("Cmd+Shift+S: captures screenshot with preview", async () => {
    setupMocks();
    render(App);
    hotkeyAction.set({ type: "screenshot-to-session", preview: true });

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("capture_app_screenshot", { cropped: false });
      expect(mocks.openPath).toHaveBeenCalledWith("/tmp/the-controller-screenshot.png");
    });
  });

  it("Cmd+D: captures cropped screenshot without preview", async () => {
    setupMocks();
    render(App);
    hotkeyAction.set({ type: "screenshot-to-session", cropped: true });

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("capture_app_screenshot", { cropped: true });
    });

    expect(mocks.openPath).not.toHaveBeenCalled();
  });

  it("Cmd+Shift+D: captures cropped screenshot with preview", async () => {
    setupMocks();
    render(App);
    hotkeyAction.set({ type: "screenshot-to-session", preview: true, cropped: true });

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("capture_app_screenshot", { cropped: true });
      expect(mocks.openPath).toHaveBeenCalledWith("/tmp/the-controller-screenshot.png");
    });
  });
});

describe("Window title updates on staging", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // @ts-expect-error compile-time constants injected in app builds
    globalThis.__BUILD_COMMIT__ = "test-commit";
    // @ts-expect-error compile-time constants injected in app builds
    globalThis.__BUILD_BRANCH__ = "test-branch";
    // @ts-expect-error compile-time constants injected in app builds
    globalThis.__DEV_PORT__ = "1420";

    projects.set([{ ...baseProject, staged_session: null, maintainer: { enabled: false, interval_minutes: 60 }, auto_worker: { enabled: false }, prompts: [] }]);
    activeSessionId.set(null);
    focusTarget.set(null);
    hotkeyAction.set(null);
    showKeyHints.set(false);
    sidebarVisible.set(true);
    onboardingComplete.set(true);
    appConfig.set({ projects_root: "/tmp/projects" });
    sessionStatuses.set(new Map());
    expandedProjects.set(new Set());
  });

  it("shows build-time info in title when no session is staged", async () => {
    vi.mocked(command).mockImplementation(async (cmd: string) => {
      if (cmd === "restore_sessions") return;
      if (cmd === "check_onboarding") return { projects_root: "/tmp/projects" };
      return;
    });

    render(App);

    await waitFor(() => {
      expect(mocks.setTitle).toHaveBeenCalledWith(
        "The Controller (test-commit, test-branch, localhost:1420)",
      );
    });
  });

  it("updates title with repo HEAD when a session is staged", async () => {
    vi.mocked(command).mockImplementation(async (cmd: string) => {
      if (cmd === "restore_sessions") return;
      if (cmd === "check_onboarding") return { projects_root: "/tmp/projects" };
      if (cmd === "get_repo_head") return ["staging/fix-foo", "abc1234"];
      return;
    });

    render(App);

    // Stage a session by updating the projects store
    projects.set([{
      ...baseProject,
      staged_session: {
        session_id: "sess-1",
        original_branch: "master",
        staging_branch: "staging/fix-foo",
      },
      maintainer: { enabled: false, interval_minutes: 60 },
      auto_worker: { enabled: false },
      prompts: [],
      sessions: [],
    }]);

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("get_repo_head", { repoPath: "/tmp/the-controller" });
      expect(mocks.setTitle).toHaveBeenCalledWith(
        "The Controller (abc1234, staging/fix-foo, localhost:1420)",
      );
    });
  });

  it("reverts title when session is unstaged", async () => {
    vi.mocked(command).mockImplementation(async (cmd: string) => {
      if (cmd === "restore_sessions") return;
      if (cmd === "check_onboarding") return { projects_root: "/tmp/projects" };
      if (cmd === "get_repo_head") return ["staging/fix-foo", "abc1234"];
      return;
    });

    render(App);

    // Stage
    projects.set([{
      ...baseProject,
      staged_session: {
        session_id: "sess-1",
        original_branch: "master",
        staging_branch: "staging/fix-foo",
      },
      maintainer: { enabled: false, interval_minutes: 60 },
      auto_worker: { enabled: false },
      prompts: [],
      sessions: [],
    }]);

    await waitFor(() => {
      expect(mocks.setTitle).toHaveBeenCalledWith(
        "The Controller (abc1234, staging/fix-foo, localhost:1420)",
      );
    });

    // Unstage
    mocks.setTitle.mockClear();
    projects.set([{
      ...baseProject,
      staged_session: null,
      maintainer: { enabled: false, interval_minutes: 60 },
      auto_worker: { enabled: false },
      prompts: [],
      sessions: [],
    }]);

    await waitFor(() => {
      expect(mocks.setTitle).toHaveBeenCalledWith(
        "The Controller (test-commit, test-branch, localhost:1420)",
      );
    });
  });
});

describe("App issue picker flow", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // @ts-expect-error compile-time constants injected in app builds
    globalThis.__BUILD_COMMIT__ = "test-commit";
    // @ts-expect-error compile-time constants injected in app builds
    globalThis.__BUILD_BRANCH__ = "test-branch";
    // @ts-expect-error compile-time constants injected in app builds
    globalThis.__DEV_PORT__ = "1420";

    projects.set([{ ...baseProject, maintainer: { enabled: false, interval_minutes: 60 }, auto_worker: { enabled: false }, prompts: [], staged_session: null }]);
    activeSessionId.set(null);
    focusTarget.set({ type: "project", projectId: "proj-1" });
    hotkeyAction.set(null);
    showKeyHints.set(false);
    sidebarVisible.set(true);
    selectedSessionProvider.set("claude");
    onboardingComplete.set(true);
    appConfig.set({ projects_root: "/tmp/projects" });
    sessionStatuses.set(new Map());
    expandedProjects.set(new Set());

    vi.mocked(command).mockImplementation(async (cmd: string) => {
      if (cmd === "restore_sessions") return;
      if (cmd === "check_onboarding") return { projects_root: "/tmp/projects" };
      if (cmd === "create_session") return "sess-new";
      if (cmd === "list_projects") {
        return {
          projects: [{ ...baseProject, maintainer: { enabled: false, interval_minutes: 60 }, auto_worker: { enabled: false }, prompts: [], staged_session: null, sessions: [] }],
          corrupt_entries: [],
        };
      }
      return;
    });
  });

  it("creates background issue sessions with codex even when the requested kind is claude", async () => {
    render(App);

    hotkeyAction.set({
      type: "pick-issue-for-session",
      projectId: "proj-1",
      repoPath: "/tmp/the-controller",
      kind: "claude",
      background: true,
    });

    await fireEvent.click(await screen.findByTestId("mock-issue-select"));

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("create_session", expect.objectContaining({
        projectId: "proj-1",
        githubIssue: expect.objectContaining({ number: 42 }),
        kind: "codex",
        background: true,
      }));
    });
  });
});

describe("App issue creation flow", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // @ts-expect-error compile-time constants injected in app builds
    globalThis.__BUILD_COMMIT__ = "test-commit";
    // @ts-expect-error compile-time constants injected in app builds
    globalThis.__BUILD_BRANCH__ = "test-branch";
    // @ts-expect-error compile-time constants injected in app builds
    globalThis.__DEV_PORT__ = "1420";

    projects.set([{ ...baseProject, maintainer: { enabled: false, interval_minutes: 60 }, auto_worker: { enabled: false }, prompts: [], staged_session: null }]);
    activeSessionId.set(null);
    focusTarget.set({ type: "project", projectId: "proj-1" });
    hotkeyAction.set(null);
    showKeyHints.set(false);
    sidebarVisible.set(true);
    selectedSessionProvider.set("claude");
    onboardingComplete.set(true);
    appConfig.set({ projects_root: "/tmp/projects" });
    sessionStatuses.set(new Map());
    expandedProjects.set(new Set());

    vi.mocked(command).mockImplementation(async (cmd: string) => {
      if (cmd === "restore_sessions") return;
      if (cmd === "check_onboarding") return { projects_root: "/tmp/projects" };
      if (cmd === "generate_issue_body") return "Generated body";
      if (cmd === "create_github_issue") {
        return {
          number: 77,
          title: "Mock issue",
          url: "https://example.com/issues/77",
          body: "Generated body",
          labels: [],
        };
      }
      if (cmd === "list_projects") {
        return {
          projects: [{ ...baseProject, maintainer: { enabled: false, interval_minutes: 60 }, auto_worker: { enabled: false }, prompts: [], staged_session: null, sessions: [] }],
          corrupt_entries: [],
        };
      }
      return;
    });
  });

  it("creates low-complexity issues with the canonical complexity:low label", async () => {
    render(App);

    hotkeyAction.set({
      type: "create-issue",
      projectId: "proj-1",
      repoPath: "/tmp/the-controller",
    });

    await fireEvent.click(await screen.findByTestId("mock-create-issue-submit"));

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("add_github_label", expect.objectContaining({
        issueNumber: 77,
        label: "complexity:low",
      }));
    });
  });
});
