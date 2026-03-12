<script lang="ts">
  import { onMount } from "svelte";
  import { fromStore } from "svelte/store";
  import { command, listen } from "$lib/backend";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Sidebar from "./lib/Sidebar.svelte";
  import TerminalManager from "./lib/TerminalManager.svelte";
  import Onboarding from "./lib/Onboarding.svelte";
  import Toast from "./lib/Toast.svelte";
  import HotkeyManager from "./lib/HotkeyManager.svelte";
  import HotkeyHelp from "./lib/HotkeyHelp.svelte";

  import CreateIssueModal from "./lib/CreateIssueModal.svelte";
  import IssuePickerModal from "./lib/IssuePickerModal.svelte";
  import PromptPickerModal from "./lib/PromptPickerModal.svelte";
  import SecureEnvModal from "./lib/SecureEnvModal.svelte";
  import DeploySetupModal from "./lib/DeploySetupModal.svelte";
  import TriagePanel from "./lib/TriagePanel.svelte";
  import AssignedIssuesPanel from "./lib/AssignedIssuesPanel.svelte";
  import KeystrokeVisualizer from "./lib/KeystrokeVisualizer.svelte";
  import WorkspaceModePicker from "./lib/WorkspaceModePicker.svelte";
  import AgentDashboard from "./lib/AgentDashboard.svelte";
  import NotesEditor from "./lib/NotesEditor.svelte";
  import ArchitectureExplorer from "./lib/ArchitectureExplorer.svelte";
  import InfrastructureDashboard from "./lib/InfrastructureDashboard.svelte";
  import { refreshProjectsFromBackend } from "./lib/project-listing";
  import { showToast } from "./lib/toast";
  import { appConfig, architectureViews, createArchitectureViewState, onboardingComplete, hotkeyAction, showKeyHints, sidebarVisible, workspaceModePickerVisible, workspaceMode, focusTarget, projects, sessionStatuses, activeSessionId, expandedProjects, dispatchHotkeyAction, focusTerminalSoon, selectedSessionProvider, type ArchitectureResult, type Config, type GithubIssue, type Project, type SavedPrompt, type SessionStatus, type TriageCategory } from "./lib/stores";
  let ready = $state(false);
  let createIssueTarget: { projectId: string; repoPath: string } | null = $state(null);
  let issuePickerTarget: { projectId: string; repoPath: string; kind?: string; background?: boolean } | null = $state(null);
  let triagePanelOpen: TriageCategory | null = $state(null);
  let assignedIssuesPanelOpen = $state(false);
  let promptPickerTarget: { projectId: string } | null = $state(null);
  let secureEnvRequest: { requestId: string; projectId: string; projectName: string; key: string } | null = $state(null);
  let deploySetupOpen = $state(false);

  const sidebarVisibleState = fromStore(sidebarVisible);
  const showKeyHintsState = fromStore(showKeyHints);

  const workspaceModePickerVisibleState = fromStore(workspaceModePickerVisible);
  const workspaceModeState = fromStore(workspaceMode);
  const onboardingCompleteState = fromStore(onboardingComplete);
  const projectsState = fromStore(projects);
  const activeSessionIdState = fromStore(activeSessionId);
  const focusTargetState = fromStore(focusTarget);
  const architectureViewsState = fromStore(architectureViews);
  const selectedSessionProviderState = fromStore(selectedSessionProvider);
  let currentSessionProvider = $derived(selectedSessionProviderState.current);
  let currentArchitectureProject = $derived.by(() => {
    const focusedProjectId =
      focusTargetState.current?.projectId ??
      projectsState.current.find((project) =>
        project.sessions.some((session) => session.id === activeSessionIdState.current),
      )?.id ??
      projectsState.current[0]?.id ??
      null;

    if (!focusedProjectId) {
      return null;
    }

    return projectsState.current.find((project) => project.id === focusedProjectId) ?? null;
  });
  let currentArchitectureView = $derived.by(() => {
    if (!currentArchitectureProject) {
      return null;
    }

    return (
      architectureViewsState.current.get(currentArchitectureProject.id) ??
      createArchitectureViewState()
    );
  });

  $effect(() => {
    const unsub = hotkeyAction.subscribe((action) => {
      if (action?.type === "toggle-help") {
        showKeyHints.update((v) => !v);
      } else if (action?.type === "create-issue") {
        createIssueTarget = { projectId: action.projectId, repoPath: action.repoPath };
      } else if (action?.type === "pick-issue-for-session") {
        issuePickerTarget = { projectId: action.projectId, repoPath: action.repoPath, kind: action.kind, background: action.background };
      } else if (action?.type === "screenshot-to-session") {
        screenshotToNewSession(action.preview ?? false, action.cropped ?? false);
      } else if (action?.type === "toggle-maintainer-enabled") {
        toggleMaintainerEnabled();
      } else if (action?.type === "toggle-auto-worker-enabled") {
        toggleAutoWorkerEnabled();
      } else if (action?.type === "toggle-triage-panel") {
        if (action.category) {
          triagePanelOpen = triagePanelOpen ? null : action.category;
        }
      } else if (action?.type === "toggle-assigned-issues-panel") {
        assignedIssuesPanelOpen = !assignedIssuesPanelOpen;
      } else if (action?.type === "save-session-prompt") {
        saveSessionPrompt(action.projectId, action.sessionId);
      } else if (action?.type === "pick-prompt-for-session") {
        promptPickerTarget = { projectId: action.projectId };
      } else if (action?.type === "generate-architecture") {
        generateArchitectureForProject(action.projectId, action.repoPath);
      } else if (action?.type === "deploy-project") {
        command<boolean>("is_deploy_provisioned").then(async (provisioned) => {
          if (!provisioned) {
            deploySetupOpen = true;
          } else {
            const project = projectsState.current.find((p) => p.id === action.projectId);
            if (!project) return;
            try {
              showToast("Deploying...", "info");
              const result = await command<{ url: string; coolify_uuid: string }>("deploy_project", {
                request: {
                  projectName: project.name,
                  repoPath: project.repo_path,
                  subdomain: project.name.toLowerCase().replace(/[^a-z0-9-]/g, "-"),
                  projectType: "node",
                },
              });
              showToast(`Deployed to ${result.url}`, "info");
            } catch (e) {
              showToast(String(e), "error");
            }
          }
        });
      }
    });
    return unsub;
  });

  function getTargetProject(): Project | undefined {
    const focus = focusTargetState.current;
    if (!focus) return undefined;
    return projectsState.current.find((p) => p.id === focus.projectId);
  }

  async function toggleMaintainerEnabled() {
    const project = getTargetProject();
    if (!project) return;
    const newEnabled = !project.maintainer.enabled;
    try {
      await command("configure_maintainer", {
        projectId: project.id,
        enabled: newEnabled,
        intervalMinutes: project.maintainer.interval_minutes,
      });
      await refreshProjectsFromBackend();
      showToast(`Maintainer ${newEnabled ? "enabled" : "disabled"}`, "info");
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function toggleAutoWorkerEnabled() {
    const focus = focusTargetState.current;
    if (!focus) return;
    const project = projectsState.current.find((p) => p.id === focus.projectId);
    if (!project) return;
    const newEnabled = !project.auto_worker.enabled;
    try {
      await command("configure_auto_worker", {
        projectId: project.id,
        enabled: newEnabled,
      });
      await refreshProjectsFromBackend();
      showToast(`Auto-worker ${newEnabled ? "enabled" : "disabled"}`, "info");
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function saveSessionPrompt(projectId: string, sessionId: string) {
    try {
      await command("save_session_prompt", { projectId, sessionId });
      showToast("Prompt saved", "info");
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function handleIssueSubmit(title: string, priority: "high" | "low", complexity: "high" | "low") {
    const repoPath = createIssueTarget!.repoPath;
    createIssueTarget = null; // close modal immediately

    try {
      showToast("Generating issue description...", "info");
      const body = await command<string>("generate_issue_body", { title });

      showToast("Creating issue...", "info");
      const issue = await command<GithubIssue>("create_github_issue", {
        repoPath,
        title,
        body,
      });

      command("add_github_label", {
        repoPath,
        issueNumber: issue.number,
        label: `priority:${priority}`,
        description: priority === "high" ? "Important, should be tackled soon" : "Nice to have, can wait",
        color: priority === "high" ? "F38BA8" : "A6E3A1",
      }).catch((e: unknown) => showToast(`Failed to add priority label: ${e}`, "error"));

      command("add_github_label", {
        repoPath,
        issueNumber: issue.number,
        label: complexity === "high" ? "complexity:high" : "complexity:low",
        description: complexity === "high" ? "Multi-step task, needs capable agents" : "Quick task, suitable for simple agents",
        color: complexity === "high" ? "FAB387" : "89DCEB",
      }).catch((e: unknown) => showToast(`Failed to add complexity label: ${e}`, "error"));

      showToast(`Issue #${issue.number} created`, "info");
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  function handleIssuePicked(issue: GithubIssue) {
    const target = issuePickerTarget!;
    issuePickerTarget = null;
    createSessionWithIssue(target.projectId, target.repoPath, issue, target.kind, target.background);
  }

  function handleIssuePickerSkip() {
    const target = issuePickerTarget!;
    issuePickerTarget = null;
    dispatchHotkeyAction({
      type: "create-session",
      projectId: target.projectId,
      kind: target.kind ?? currentSessionProvider,
    });
  }

  async function handlePromptPicked(prompt: SavedPrompt) {
    const target = promptPickerTarget!;
    promptPickerTarget = null;

    const wrappedPrompt = `You are a prompt engineer, here is a prompt, your goal is to collaborate with me to make it better:\n\n<prompt>\n${prompt.text}\n</prompt>`;

    try {
      const sessionId: string = await command("create_session", {
        projectId: target.projectId,
        kind: "claude",
        initialPrompt: wrappedPrompt,
      });
      await activateNewSession(sessionId, target.projectId);
      focusTerminalSoon();
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function activateNewSession(sessionId: string, projectId: string) {
    sessionStatuses.update((m: Map<string, SessionStatus>) => {
      const next = new Map(m);
      next.set(sessionId, "working");
      return next;
    });
    activeSessionId.set(sessionId);
    await refreshProjectsFromBackend();
    expandedProjects.update((s: Set<string>) => {
      const next = new Set(s);
      next.add(projectId);
      return next;
    });
  }

  async function createSessionWithIssue(projectId: string, repoPath: string, issue: GithubIssue, kind?: string, background?: boolean) {
    try {
      const sessionId: string = await command("create_session", {
        projectId,
        githubIssue: issue,
        kind: background ? "codex" : (kind ?? currentSessionProvider),
        background: background ?? false,
      });
      // Post comment on the issue (fire and forget)
      command("post_github_comment", {
        repoPath,
        issueNumber: issue.number,
        body: `Working on this in session \`${sessionId.substring(0, 8)}\``,
      }).catch((e: unknown) => showToast(`Failed to post comment: ${e}`, "error"));
      // Add in-progress label (fire and forget)
      command("add_github_label", {
        repoPath,
        issueNumber: issue.number,
        label: "in-progress",
      }).catch((e: unknown) => showToast(`Failed to add label: ${e}`, "error"));

      await activateNewSession(sessionId, projectId);
      focusTerminalSoon();
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function screenshotToNewSession(preview: boolean, cropped: boolean) {
    // IMPORTANT: Screenshot sessions are a core personalization feature — they let
    // users debug and modify the controller from within itself. This must always
    // target the controller project, never the focused project.
    const project = projectsState.current.find((p) => p.name === "the-controller");

    if (!project) {
      showToast("The controller project must be loaded to use screenshot sessions", "error");
      return;
    }

    try {
      // 1. Capture screenshot
      showToast(cropped ? "Select area to capture..." : "Capturing screenshot...", "info");
      const screenshotPath: string = await command("capture_app_screenshot", { cropped });

      // Open in Preview only when preview is requested
      if (preview) {
        import("@tauri-apps/plugin-opener").then(({ openPath }) => openPath(screenshotPath));
      }

      // 2. Create a new session with initial prompt referencing the screenshot file.
      // Tell Claude to share the path and wait for further instructions.
      const sessionId: string = await command("create_session", {
        projectId: project.id,
        kind: currentSessionProvider,
        initialPrompt: `I just took a screenshot of the app. The screenshot is saved at: ${screenshotPath}\nPlease read the screenshot image and share what you see, but wait for further prompts before taking any action.`,
      });

      await activateNewSession(sessionId, project.id);
      focusTerminalSoon();
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  function updateWindowTitle(branch: string, commit: string) {
    try {
      const parts = [commit, branch, `localhost:${__DEV_PORT__}`];
      const title = `The Controller (${parts.join(", ")})`;
      getCurrentWindow().setTitle(title);
    } catch {
      // Browser mode — no Tauri window API available
    }
  }

  function handleArchitectureSelection(componentId: string) {
    if (!currentArchitectureProject) {
      return;
    }

    architectureViews.update((views) => {
      const next = new Map(views);
      const currentView =
        next.get(currentArchitectureProject.id) ?? createArchitectureViewState();
      next.set(currentArchitectureProject.id, {
        ...currentView,
        selectedComponentId: componentId,
      });
      return next;
    });
  }

  async function generateArchitectureForProject(projectId: string, repoPath: string) {
    if (architectureViewsState.current.get(projectId)?.isGenerating) {
      return;
    }

    architectureViews.update((views) => {
      const next = new Map(views);
      const currentView = next.get(projectId) ?? createArchitectureViewState();
      next.set(projectId, {
        ...currentView,
        isGenerating: true,
        error: null,
      });
      return next;
    });

    try {
      const result = await command<ArchitectureResult>("generate_architecture", { repoPath });
      architectureViews.update((views) => {
        const next = new Map(views);
        const currentView = next.get(projectId) ?? createArchitectureViewState();
        const selectedComponentId =
          currentView.selectedComponentId &&
          result.components.some((component) => component.id === currentView.selectedComponentId)
            ? currentView.selectedComponentId
            : result.components[0]?.id ?? null;

        next.set(projectId, {
          result,
          selectedComponentId,
          isGenerating: false,
          error: null,
        });
        return next;
      });
    } catch (error) {
      architectureViews.update((views) => {
        const next = new Map(views);
        const currentView = next.get(projectId) ?? createArchitectureViewState();
        next.set(projectId, {
          ...currentView,
          isGenerating: false,
          error: String(error),
        });
        return next;
      });
      showToast(`Failed to generate architecture: ${error}`, "error");
    }
  }

  onMount(() => {
    const unlistenSecureEnv = listen<string>("secure-env-requested", (payload) => {
      try {
        secureEnvRequest = JSON.parse(payload);
      } catch (e) {
        showToast(`Invalid secure env request payload: ${e}`, "error");
      }
    });

    void (async () => {
      updateWindowTitle(__BUILD_BRANCH__, __BUILD_COMMIT__);

      try {
        // Re-spawn PTY sessions for persisted active sessions
        await command("restore_sessions");

        const config = await command<Config | null>("check_onboarding");
        if (config) {
          appConfig.set(config);
          onboardingComplete.set(true);
        }
      } catch (e) {
        // Config check failed, show onboarding
      }
      ready = true;
    })();

    return () => {
      unlistenSecureEnv();
    };
  });

  async function submitSecureEnvValue(value: string) {
    if (!secureEnvRequest) return;

    const target = secureEnvRequest;
    secureEnvRequest = null;

    try {
      await command("submit_secure_env_value", {
        requestId: target.requestId,
        value,
      });
      showToast(`Saved ${target.key} for ${target.projectName}`, "info");
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function cancelSecureEnvRequest() {
    if (!secureEnvRequest) return;

    const target = secureEnvRequest;
    secureEnvRequest = null;

    try {
      await command("cancel_secure_env_request", {
        requestId: target.requestId,
      });
    } catch (e) {
      showToast(String(e), "error");
    }
  }
</script>

{#if ready}
  {#if !onboardingCompleteState.current}
    <Onboarding />
  {:else}
    <div class="app-layout">
      {#if sidebarVisibleState.current}
        <Sidebar />
      {/if}
      <main class="terminal-area">
        {#if workspaceModeState.current === "agents"}
          <AgentDashboard />
        {:else if workspaceModeState.current === "architecture"}
          <ArchitectureExplorer
            projectName={currentArchitectureProject?.name ?? "Architecture"}
            architecture={currentArchitectureView?.result ?? null}
            selectedComponentId={currentArchitectureView?.selectedComponentId ?? null}
            onSelectComponent={handleArchitectureSelection}
            onGenerateArchitecture={() => {
              if (currentArchitectureProject) {
                generateArchitectureForProject(
                  currentArchitectureProject.id,
                  currentArchitectureProject.repo_path,
                );
              }
            }}
            isGenerating={currentArchitectureView?.isGenerating ?? false}
            error={currentArchitectureView?.error ?? null}
          />
        {:else if workspaceModeState.current === "notes"}
          <NotesEditor />
        {:else if workspaceModeState.current === "infrastructure"}
          <InfrastructureDashboard />
        {:else}
          <TerminalManager />
        {/if}
      </main>
    </div>
    <HotkeyManager />
    {#if showKeyHintsState.current}
      <HotkeyHelp onClose={() => showKeyHints.set(false)} />
    {/if}
    {#if createIssueTarget}
      <CreateIssueModal
        onSubmit={handleIssueSubmit}
        onClose={() => { createIssueTarget = null; }}
      />
    {/if}
    {#if issuePickerTarget}
      <IssuePickerModal
        repoPath={issuePickerTarget.repoPath}
        onSelect={handleIssuePicked}
        onSkip={handleIssuePickerSkip}
        onClose={() => { issuePickerTarget = null; }}
      />
    {/if}
    {#if promptPickerTarget}
      <PromptPickerModal
        projectId={promptPickerTarget.projectId}
        onSelect={handlePromptPicked}
        onClose={() => { promptPickerTarget = null; }}
      />
    {/if}
    {#if secureEnvRequest}
      <SecureEnvModal
        projectName={secureEnvRequest.projectName}
        envKey={secureEnvRequest.key}
        onSubmit={submitSecureEnvValue}
        onClose={cancelSecureEnvRequest}
      />
    {/if}
    {#if triagePanelOpen}
      <TriagePanel category={triagePanelOpen} onClose={() => { triagePanelOpen = null; }} />
    {/if}
    {#if assignedIssuesPanelOpen}
      <AssignedIssuesPanel onClose={() => { assignedIssuesPanelOpen = false; }} />
    {/if}
    {#if workspaceModePickerVisibleState.current}
      <WorkspaceModePicker />
    {/if}
    {#if deploySetupOpen}
      <DeploySetupModal
        onComplete={() => { deploySetupOpen = false; showToast("Deploy setup complete", "info"); }}
        onClose={() => { deploySetupOpen = false; }}
      />
    {/if}
  {/if}
{/if}
<KeystrokeVisualizer />
<Toast />

<style>
  .app-layout {
    display: flex;
    height: 100vh;
    width: 100vw;
    background: var(--bg-void);
    overflow: hidden;
  }
  .terminal-area {
    flex: 1;
    overflow: hidden;
  }
</style>
