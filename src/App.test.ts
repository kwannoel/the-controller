import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { get } from "svelte/store";
import { command, listen } from "$lib/backend";
import {
  appConfig,
  expandedProjects,
  focusTarget,
  hotkeyAction,
  onboardingComplete,
  projects,
  showKeyHints,
  sidebarVisible,
  workspaceMode,
  workspaceModePickerVisible,
  type Project,
} from "./lib/stores";

const componentMocks = vi.hoisted(() => ({
  sidebar: vi.fn(),
  onboarding: vi.fn(),
  toast: vi.fn(),
  hotkeyManager: vi.fn(),
  hotkeyHelp: vi.fn(),
  keystrokeVisualizer: vi.fn(),
  workspaceModePicker: vi.fn(),
  agentDashboard: vi.fn(),
  agentCreationWorkspace: vi.fn(),
  kanbanBoard: vi.fn(),
  chatWorkspace: vi.fn(),
}));

vi.mock("./lib/Sidebar.svelte", () => ({ default: componentMocks.sidebar }));
vi.mock("./lib/Onboarding.svelte", () => ({ default: componentMocks.onboarding }));
vi.mock("./lib/Toast.svelte", () => ({ default: componentMocks.toast }));
vi.mock("./lib/HotkeyManager.svelte", () => ({ default: componentMocks.hotkeyManager }));
vi.mock("./lib/HotkeyHelp.svelte", () => ({ default: componentMocks.hotkeyHelp }));
vi.mock("./lib/KeystrokeVisualizer.svelte", () => ({ default: componentMocks.keystrokeVisualizer }));
vi.mock("./lib/WorkspaceModePicker.svelte", () => ({ default: componentMocks.workspaceModePicker }));
vi.mock("./lib/AgentDashboard.svelte", () => ({ default: componentMocks.agentDashboard }));
vi.mock("./lib/agents/AgentCreationWorkspace.svelte", () => ({ default: componentMocks.agentCreationWorkspace }));
vi.mock("./lib/KanbanBoard.svelte", () => ({ default: componentMocks.kanbanBoard }));
vi.mock("./lib/chat/ChatWorkspace.svelte", () => ({ default: componentMocks.chatWorkspace }));
vi.mock("./lib/toast", () => ({
  showToast: vi.fn(),
}));

import App from "./App.svelte";
import { showToast } from "./lib/toast";

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
  staged_sessions: [],
};

function resetStores() {
  projects.set([baseProject]);
  focusTarget.set({ type: "project", projectId: "proj-1" });
  hotkeyAction.set(null);
  showKeyHints.set(false);
  sidebarVisible.set(true);
  workspaceMode.set("chat");
  workspaceModePickerVisible.set(false);
  onboardingComplete.set(true);
  appConfig.set({ projects_root: "/tmp/projects" });
  expandedProjects.set(new Set());
}

function mockDefaultCommands() {
  vi.mocked(command).mockImplementation(async (cmd: string) => {
    if (cmd === "check_onboarding") return { projects_root: "/tmp/projects" };
    return undefined;
  });
}

describe("App shell", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // @ts-expect-error compile-time constants injected in app builds
    globalThis.__BUILD_COMMIT__ = "test-commit";
    // @ts-expect-error compile-time constants injected in app builds
    globalThis.__BUILD_BRANCH__ = "test-branch";
    // @ts-expect-error compile-time constants injected in app builds
    globalThis.__DEV_PORT__ = "1420";
    vi.mocked(listen).mockReturnValue(() => {});
    mockDefaultCommands();
    resetStores();
  });

  it("opens in chat mode by default and updates the window title", async () => {
    render(App);

    await waitFor(() => {
      expect(componentMocks.chatWorkspace).toHaveBeenCalled();
    });

    expect(componentMocks.agentDashboard).not.toHaveBeenCalled();
    expect(componentMocks.kanbanBoard).not.toHaveBeenCalled();
    expect(document.title).toBe("The Controller (test-commit, test-branch, localhost:1420)");
  });

  it("renders the agents, agent creation, observe placeholder, and kanban workspaces", async () => {
    workspaceMode.set("agents");
    render(App);
    await waitFor(() => {
      expect(componentMocks.agentDashboard).toHaveBeenCalled();
    });

    vi.clearAllMocks();
    workspaceMode.set("agent-create");
    render(App);
    await waitFor(() => {
      expect(componentMocks.agentCreationWorkspace).toHaveBeenCalled();
    });

    vi.clearAllMocks();
    workspaceMode.set("agent-observe");
    render(App);
    await waitFor(() => {
      expect(screen.getAllByText("Agent Observe").length).toBeGreaterThan(0);
    });
    expect(componentMocks.chatWorkspace).not.toHaveBeenCalled();

    vi.clearAllMocks();
    workspaceMode.set("kanban");
    render(App);
    await waitFor(() => {
      expect(componentMocks.kanbanBoard).toHaveBeenCalled();
    });

    expect(componentMocks.chatWorkspace).not.toHaveBeenCalled();
  });

  it("shows onboarding when config is missing", async () => {
    onboardingComplete.set(false);
    appConfig.set(null);
    vi.mocked(command).mockImplementation(async (cmd: string) => {
      if (cmd === "check_onboarding") return null;
      return undefined;
    });

    render(App);

    await waitFor(() => {
      expect(componentMocks.onboarding).toHaveBeenCalled();
    });
  });

  it("loads a project from fuzzy finder selection", async () => {
    const loadedProject = { ...baseProject, id: "proj-loaded", name: "alpha-project", repo_path: "/tmp/alpha" };
    vi.mocked(command).mockImplementation(async (cmd: string) => {
      if (cmd === "check_onboarding") return { projects_root: "/tmp/projects" };
      if (cmd === "list_root_directories") return [{ name: "alpha-project", path: "/tmp/alpha" }];
      if (cmd === "load_project") return loadedProject;
      if (cmd === "list_projects") return { projects: [loadedProject], corrupt_entries: [] };
      return undefined;
    });

    render(App);
    hotkeyAction.set({ type: "open-fuzzy-finder" });

    await fireEvent.click(await screen.findByText("alpha-project"));

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("load_project", { name: "alpha-project", repoPath: "/tmp/alpha" });
      expect(get(focusTarget)).toEqual({ type: "project", projectId: "proj-loaded" });
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
    mockDefaultCommands();
    resetStores();
  });

  it("submits secure env values without leaking the secret to toast text", async () => {
    let secureEnvHandler: ((payload: string) => void) | undefined;
    vi.mocked(listen).mockImplementation((event: string, handler: (payload: string) => void) => {
      if (event === "secure-env-requested") secureEnvHandler = handler;
      return () => {};
    });
    vi.mocked(command).mockImplementation(async (cmd: string) => {
      if (cmd === "check_onboarding") return { projects_root: "/tmp/projects" };
      if (cmd === "submit_secure_env_value") return "created";
      return undefined;
    });

    render(App);
    secureEnvHandler?.(JSON.stringify({
      requestId: "req-123",
      projectId: "proj-1",
      projectName: "demo-project",
      key: "OPENAI_API_KEY",
    }));

    const input = await screen.findByLabelText("Secret value");
    await userEvent.type(input, "new-secret");
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("submit_secure_env_value", {
        requestId: "req-123",
        value: "new-secret",
      });
    });

    expect(showToast).toHaveBeenCalledWith("Saved OPENAI_API_KEY for demo-project", "info");
    expect(showToast).not.toHaveBeenCalledWith(expect.stringContaining("new-secret"), expect.anything());
  });

  it("cancels secure env requests", async () => {
    let secureEnvHandler: ((payload: string) => void) | undefined;
    vi.mocked(listen).mockImplementation((event: string, handler: (payload: string) => void) => {
      if (event === "secure-env-requested") secureEnvHandler = handler;
      return () => {};
    });
    vi.mocked(command).mockImplementation(async (cmd: string) => {
      if (cmd === "check_onboarding") return { projects_root: "/tmp/projects" };
      if (cmd === "cancel_secure_env_request") return undefined;
      return undefined;
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
