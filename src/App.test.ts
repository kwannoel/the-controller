import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { command, listen } from "$lib/backend";
import {
  activeSessionId,
  appConfig,
  architectureViews,
  createArchitectureViewState,
  expandedProjects,
  focusTarget,
  hotkeyAction,
  onboardingComplete,
  projects,
  selectedSessionProvider,
  sessionStatuses,
  showKeyHints,
  sidebarVisible,
  workspaceMode,
  type ArchitectureResult,
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
vi.mock("mermaid", () => ({
  default: {
    initialize: vi.fn(),
    render: vi.fn(async (diagramId: string) => ({
      svg: `
        <svg id="${diagramId}" viewBox="0 0 100 100">
          <g class="node" id="flowchart-ui-0">
            <rect />
            <text>UI</text>
          </g>
          <g class="node" id="flowchart-backend-1">
            <rect />
            <text>Backend</text>
          </g>
        </svg>
      `,
    })),
  },
}));

vi.mock("./lib/IssuesModal.svelte", async () => ({
  default: (await import("./test/IssuesModalMock.svelte")).default,
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

const secondProject: Project = {
  ...baseProject,
  id: "proj-2",
  name: "client-app",
  repo_path: "/tmp/client-app",
  sessions: [
    {
      id: "sess-2",
      label: "session-2",
      worktree_path: null,
      worktree_branch: null,
      archived: false,
      kind: "claude",
      github_issue: null,
      initial_prompt: null,
      auto_worker_session: false,
    },
  ],
};

const generatedArchitecture: ArchitectureResult = {
  title: "Generated Architecture",
  mermaid: "flowchart TD\nui[UI] --> backend[Backend]",
  components: [
    {
      id: "ui",
      name: "UI Shell",
      summary: "Hosts the workspace shell.",
      contains: ["App.svelte"],
      incoming_relationships: [],
      outgoing_relationships: [
        {
          component_id: "backend",
          summary: "Requests architecture generation.",
        },
      ],
      evidence_paths: ["src/App.svelte"],
      evidence_snippets: ["workspaceModeState.current === \"architecture\""],
    },
    {
      id: "backend",
      name: "Backend Command Layer",
      summary: "Runs architecture analysis.",
      contains: ["commands.rs"],
      incoming_relationships: [
        {
          component_id: "ui",
          summary: "Serves architecture payloads back to the UI shell.",
        },
      ],
      outgoing_relationships: [],
      evidence_paths: ["src-tauri/src/commands.rs"],
      evidence_snippets: ["pub async fn generate_architecture"],
    },
  ],
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
    workspaceMode.set("development");
    architectureViews.set(new Map());

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
      if (cmd === "connect_session") return;
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

  it("Cmd+S (direct): captures screenshot and spawns session for the-controller", async () => {
    setupMocks();
    render(App);
    hotkeyAction.set({ type: "screenshot-to-session", direct: true });

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("capture_app_screenshot", { cropped: false });
    });

    // Should directly create session without showing picker
    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("create_session", expect.objectContaining({
        projectId: "proj-1",
        kind: "claude",
        initialPrompt: expect.stringContaining("/tmp/the-controller-screenshot.png"),
      }));
    });

    expect(screen.queryByText("Send Screenshot To")).not.toBeInTheDocument();
  });

  it("Cmd+D (direct): captures cropped screenshot and spawns session for the-controller", async () => {
    setupMocks();
    render(App);
    hotkeyAction.set({ type: "screenshot-to-session", direct: true, cropped: true });

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("capture_app_screenshot", { cropped: true });
    });

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("create_session", expect.objectContaining({
        projectId: "proj-1",
        kind: "claude",
        initialPrompt: expect.stringContaining("/tmp/the-controller-screenshot.png"),
      }));
    });

    expect(screen.queryByText("Send Screenshot To")).not.toBeInTheDocument();
  });

  it("Cmd+Shift+S (picker): captures screenshot and shows session picker", async () => {
    setupMocks();
    render(App);
    hotkeyAction.set({ type: "screenshot-to-session" });

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("capture_app_screenshot", { cropped: false });
    });

    // Session picker modal should appear
    await waitFor(() => {
      expect(screen.getByText("Send Screenshot To")).toBeInTheDocument();
      expect(screen.getByText("+ New session")).toBeInTheDocument();
    });
  });

  it("new session option in picker routes to the-controller project", async () => {
    const controllerProject = { ...baseProject, id: "proj-controller", name: "the-controller", repo_path: "/tmp/the-controller" };
    const otherProject = { ...baseProject, id: "proj-other", name: "client-app", repo_path: "/tmp/client-app" };
    setupMocks();
    projects.set([otherProject, controllerProject]);
    focusTarget.set({ type: "project", projectId: "proj-other" });

    render(App);
    hotkeyAction.set({ type: "screenshot-to-session" });

    await waitFor(() => {
      expect(screen.getByText("Send Screenshot To")).toBeInTheDocument();
    });

    // Click "+ New session" — should create session in the-controller project
    await fireEvent.click(screen.getByText("+ New session"));

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("create_session", expect.objectContaining({
        projectId: "proj-controller",
        kind: "claude",
        initialPrompt: expect.stringContaining("/tmp/the-controller-screenshot.png"),
      }));
    });
  });

  it("sends screenshot to an existing session when picked", async () => {
    const projectWithSession = {
      ...baseProject,
      sessions: [
        {
          id: "sess-existing",
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
    };
    setupMocks();
    projects.set([projectWithSession]);

    render(App);
    hotkeyAction.set({ type: "screenshot-to-session" });

    await waitFor(() => {
      expect(screen.getByText("Send Screenshot To")).toBeInTheDocument();
    });

    // Click the existing session
    await fireEvent.click(screen.getByText("session-1"));

    await waitFor(() => {
      // Should connect the PTY first, then write
      expect(command).toHaveBeenCalledWith("connect_session", expect.objectContaining({
        sessionId: "sess-existing",
      }));
      expect(command).toHaveBeenCalledWith("write_to_pty", expect.objectContaining({
        sessionId: "sess-existing",
        data: expect.stringContaining("/tmp/the-controller-screenshot.png"),
      }));
    });
  });

  it("Cmd+Shift+D (picker): captures cropped screenshot and shows picker", async () => {
    setupMocks();
    render(App);
    hotkeyAction.set({ type: "screenshot-to-session", cropped: true });

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("capture_app_screenshot", { cropped: true });
    });

    await waitFor(() => {
      expect(screen.getByText("Send Screenshot To")).toBeInTheDocument();
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

  it("does not change title when a session is staged", async () => {
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

    // Stage a session — title should NOT change
    mocks.setTitle.mockClear();
    projects.set([{
      ...baseProject,
      staged_session: {
        session_id: "sess-1",
        pid: 12345,
        port: 1421,
      },
      maintainer: { enabled: false, interval_minutes: 60 },
      auto_worker: { enabled: false },
      prompts: [],
      sessions: [{ id: "sess-1", label: "fix-foo", worktree_path: null, worktree_branch: null, archived: false, kind: "claude", github_issue: null, initial_prompt: null, auto_worker_session: false }],
    }]);

    // Give reactivity a tick, then verify title was NOT updated
    await new Promise((r) => setTimeout(r, 50));
    expect(mocks.setTitle).not.toHaveBeenCalled();
  });
});

describe("App architecture workspace", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // @ts-expect-error compile-time constants injected in app builds
    globalThis.__BUILD_COMMIT__ = "test-commit";
    // @ts-expect-error compile-time constants injected in app builds
    globalThis.__BUILD_BRANCH__ = "test-branch";
    // @ts-expect-error compile-time constants injected in app builds
    globalThis.__DEV_PORT__ = "1420";

    projects.set([baseProject, secondProject]);
    activeSessionId.set(null);
    focusTarget.set({ type: "project", projectId: "proj-1" });
    hotkeyAction.set(null);
    showKeyHints.set(false);
    sidebarVisible.set(true);
    selectedSessionProvider.set("claude");
    workspaceMode.set("architecture");
    architectureViews.set(new Map());
    onboardingComplete.set(true);
    appConfig.set({ projects_root: "/tmp/projects" });
    sessionStatuses.set(new Map());
    expandedProjects.set(new Set());
  });

  it("generates architecture for the focused project", async () => {
    focusTarget.set({ type: "project", projectId: "proj-2" });

    vi.mocked(command).mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "restore_sessions") return;
      if (cmd === "check_onboarding") return { projects_root: "/tmp/projects" };
      if (cmd === "generate_architecture") {
        expect(args).toEqual({ repoPath: "/tmp/client-app" });
        return generatedArchitecture;
      }
      return;
    });

    render(App);

    const generateButton = await waitFor(() =>
      screen.getByRole("button", { name: "Generate" }),
    );

    await fireEvent.click(generateButton);

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("generate_architecture", { repoPath: "/tmp/client-app" });
    });

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "UI Shell" })).toBeInTheDocument();
    });
  });

  it("re-generates architecture without breaking the selected component", async () => {
    architectureViews.set(new Map([
      [
        "proj-1",
        {
          ...createArchitectureViewState(generatedArchitecture),
          selectedComponentId: "backend",
        },
      ],
    ]));

    const refreshedArchitecture: ArchitectureResult = {
      ...generatedArchitecture,
      title: "Refreshed Architecture",
      components: [
        generatedArchitecture.components[0],
        {
          ...generatedArchitecture.components[1],
          summary: "Runs architecture analysis and refreshes the cached view.",
        },
      ],
    };

    vi.mocked(command).mockImplementation(async (cmd: string) => {
      if (cmd === "restore_sessions") return;
      if (cmd === "check_onboarding") return { projects_root: "/tmp/projects" };
      if (cmd === "generate_architecture") return refreshedArchitecture;
      return;
    });

    render(App);

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Backend Command Layer" })).toBeInTheDocument();
    });

    await fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("generate_architecture", {
        repoPath: "/tmp/the-controller",
      });
    });

    await waitFor(() => {
      expect(screen.getByText("Refreshed Architecture")).toBeInTheDocument();
    });

    expect(screen.getByRole("heading", { name: "Backend Command Layer" })).toBeInTheDocument();
    expect(
      screen.getByText("Runs architecture analysis and refreshes the cached view."),
    ).toBeInTheDocument();
  });

  it("ignores duplicate architecture generation requests while one is already running", async () => {
    let generateArchitectureCalls = 0;
    let resolveArchitecture: ((value: ArchitectureResult) => void) | undefined;
    const pendingArchitecture = new Promise<ArchitectureResult>((resolve) => {
      resolveArchitecture = resolve;
    });

    vi.mocked(command).mockImplementation(async (cmd: string) => {
      if (cmd === "restore_sessions") return;
      if (cmd === "check_onboarding") return { projects_root: "/tmp/projects" };
      if (cmd === "generate_architecture") {
        generateArchitectureCalls += 1;
        return pendingArchitecture;
      }
      return;
    });

    render(App);

    hotkeyAction.set({
      type: "generate-architecture",
      projectId: "proj-1",
      repoPath: "/tmp/the-controller",
    });

    await waitFor(() => {
      expect(generateArchitectureCalls).toBe(1);
    });

    hotkeyAction.set({
      type: "generate-architecture",
      projectId: "proj-1",
      repoPath: "/tmp/the-controller",
    });

    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(generateArchitectureCalls).toBe(1);

    resolveArchitecture?.(generatedArchitecture);

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "UI Shell" })).toBeInTheDocument();
    });
  });
});

describe("App issue assign flow", () => {
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

  it("creates a session when assigning an issue from the issues modal", async () => {
    render(App);

    hotkeyAction.set({
      type: "open-issues-modal",
      projectId: "proj-1",
      repoPath: "/tmp/the-controller",
    });

    await fireEvent.click(await screen.findByTestId("mock-issue-assign"));

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("create_session", expect.objectContaining({
        projectId: "proj-1",
        githubIssue: expect.objectContaining({ number: 42 }),
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
      type: "open-issues-modal",
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

describe("App secure env flow", () => {
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

    vi.mocked(command).mockImplementation(async (cmd: string) => {
      if (cmd === "restore_sessions") return;
      if (cmd === "check_onboarding") return { projects_root: "/tmp/projects" };
      if (cmd === "submit_secure_env_value") return "created";
      if (cmd === "cancel_secure_env_request") return;
      return;
    });
  });

  it("opens the secure env modal from the backend event and submits without leaking the secret to toast text", async () => {
    let secureEnvHandler: ((payload: string) => void) | undefined;
    vi.mocked(listen).mockImplementation((event: string, handler: (payload: string) => void) => {
      if (event === "secure-env-requested") secureEnvHandler = handler;
      return () => {};
    });

    render(App);

    secureEnvHandler?.(JSON.stringify({
      requestId: "req-123",
      projectId: "proj-1",
      projectName: "demo-project",
      key: "OPENAI_API_KEY",
    }));

    const input = await screen.findByLabelText("Secret value");
    await fireEvent.input(input, { target: { value: "new-secret" } });
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("submit_secure_env_value", {
        requestId: "req-123",
        value: "new-secret",
      });
    });

    const { showToast } = await import("./lib/toast");
    expect(showToast).toHaveBeenCalledWith("Saved OPENAI_API_KEY for demo-project", "info");
    expect(showToast).not.toHaveBeenCalledWith(expect.stringContaining("new-secret"), expect.anything());
  });

  it("cancels the secure env request from the modal", async () => {
    let secureEnvHandler: ((payload: string) => void) | undefined;
    vi.mocked(listen).mockImplementation((event: string, handler: (payload: string) => void) => {
      if (event === "secure-env-requested") secureEnvHandler = handler;
      return () => {};
    });

    render(App);

    secureEnvHandler?.(JSON.stringify({
      requestId: "req-123",
      projectId: "proj-1",
      projectName: "demo-project",
      key: "OPENAI_API_KEY",
    }));

    await fireEvent.click(await screen.findByRole("button", { name: "Cancel" }));

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("cancel_secure_env_request", {
        requestId: "req-123",
      });
    });
  });
});
