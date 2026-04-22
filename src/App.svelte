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

  import IssuesModal from "./lib/IssuesModal.svelte";
  import PromptPickerModal from "./lib/PromptPickerModal.svelte";
  import SecureEnvModal from "./lib/SecureEnvModal.svelte";
  import SessionPickerModal from "./lib/SessionPickerModal.svelte";
  import FuzzyFinder from "./lib/FuzzyFinder.svelte";
  import KeystrokeVisualizer from "./lib/KeystrokeVisualizer.svelte";
  import WorkspaceModePicker from "./lib/WorkspaceModePicker.svelte";
  import AgentDashboard from "./lib/AgentDashboard.svelte";
  import KanbanBoard from "./lib/KanbanBoard.svelte";
  import ChatWorkspace from "./lib/chat/ChatWorkspace.svelte";
  import { refreshProjectsFromBackend } from "./lib/project-listing";
  import { showToast } from "./lib/toast";
  import { appConfig, onboardingComplete, hotkeyAction, showKeyHints, sidebarVisible, workspaceModePickerVisible, workspaceMode, focusTarget, projects, sessionStatuses, activeSessionId, expandedProjects, dispatchHotkeyAction, focusTerminalSoon, selectedSessionProvider, type Config, type GithubIssue, type Project, type SavedPrompt, type SessionStatus } from "./lib/stores";
  let ready = $state(false);
  let issuesModalTarget: { projectId: string; repoPath: string } | null = $state(null);
  let promptPickerTarget: { projectId: string } | null = $state(null);
  let secureEnvRequest: { requestId: string; projectId: string; projectName: string; key: string } | null = $state(null);
  let showFuzzyFinder = $state(false);
  let screenshotPickerState: { path: string; preview: boolean } | null = $state(null);

  const sidebarVisibleState = fromStore(sidebarVisible);
  const showKeyHintsState = fromStore(showKeyHints);

  const workspaceModePickerVisibleState = fromStore(workspaceModePickerVisible);
  const workspaceModeState = fromStore(workspaceMode);
  const onboardingCompleteState = fromStore(onboardingComplete);
  const projectsState = fromStore(projects);
  const activeSessionIdState = fromStore(activeSessionId);
  const focusTargetState = fromStore(focusTarget);
  const selectedSessionProviderState = fromStore(selectedSessionProvider);
  let currentSessionProvider = $derived(selectedSessionProviderState.current);

  $effect(() => {
    const unsub = hotkeyAction.subscribe((action) => {
      if (action?.type === "open-fuzzy-finder") {
        showFuzzyFinder = true;
      } else if (action?.type === "toggle-help") {
        showKeyHints.update((v) => !v);
      } else if (action?.type === "open-issues-modal") {
        issuesModalTarget = { projectId: action.projectId, repoPath: action.repoPath };
      } else if (action?.type === "assign-issue-to-session") {
        createSessionWithIssue(action.projectId, action.repoPath, action.issue);
      } else if (action?.type === "screenshot-to-session") {
        captureScreenshot(action.direct ?? false, action.cropped ?? false);
      } else if (action?.type === "toggle-maintainer-enabled") {
        toggleMaintainerEnabled();
      } else if (action?.type === "toggle-auto-worker-enabled") {
        toggleAutoWorkerEnabled();
      } else if (action?.type === "save-session-prompt") {
        saveSessionPrompt(action.projectId, action.sessionId);
      } else if (action?.type === "pick-prompt-for-session") {
        promptPickerTarget = { projectId: action.projectId };
      }
    });
    return unsub;
  });

  function getTargetProject(): Project | undefined {
    const focus = focusTargetState.current;
    if (!focus || !("projectId" in focus)) return undefined;
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
    if (!focus || !("projectId" in focus)) return;
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
    const repoPath = issuesModalTarget!.repoPath;
    issuesModalTarget = null; // close modal immediately

    try {
      showToast("Generating issue description...", "info");
      const body = await command<string>("generate_issue_body", { repoPath, title });

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

  function handleAssignIssue(issue: GithubIssue) {
    const target = issuesModalTarget!;
    issuesModalTarget = null;
    createSessionWithIssue(target.projectId, target.repoPath, issue);
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

  function screenshotPrompt(path: string): string {
    return `I just took a screenshot of the app. The screenshot is saved at: ${path}\nPlease read the screenshot image and share what you see, but wait for further prompts before taking any action.`;
  }

  async function captureScreenshot(direct: boolean, cropped: boolean) {
    try {
      showToast(cropped ? "Select area to capture..." : "Capturing screenshot...", "info");
      const screenshotPath: string = await command("capture_app_screenshot", { cropped });

      if (direct) {
        await createScreenshotSession(screenshotPath);
      } else {
        // Show session picker
        screenshotPickerState = { path: screenshotPath, preview: false };
      }
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function sendScreenshotToSession(sessionId: string, projectId: string) {
    if (!screenshotPickerState) return;
    const path = screenshotPickerState.path;
    screenshotPickerState = null;

    try {
      // Ensure the PTY is spawned before writing (it may not be active yet)
      await command("connect_session", { sessionId, rows: 24, cols: 80 });
      await command("write_to_pty", { sessionId, data: path + "\n" });
      activeSessionId.set(sessionId);
      expandedProjects.update((s: Set<string>) => {
        const next = new Set(s);
        next.add(projectId);
        return next;
      });
      focusTerminalSoon();
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function createScreenshotSession(screenshotPath: string) {
    const project = projectsState.current.find((p) => p.name === "the-controller");
    if (!project) {
      showToast("The controller project must be loaded to use screenshot sessions", "error");
      return;
    }

    try {
      const sessionId: string = await command("create_session", {
        projectId: project.id,
        kind: currentSessionProvider,
        initialPrompt: screenshotPrompt(screenshotPath),
      });
      await activateNewSession(sessionId, project.id);
      focusTerminalSoon();
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function sendScreenshotToNewSession() {
    if (!screenshotPickerState) return;
    const path = screenshotPickerState.path;
    screenshotPickerState = null;
    await createScreenshotSession(path);
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
        {:else if workspaceModeState.current === "kanban"}
          <KanbanBoard />
        {:else if workspaceModeState.current === "chat"}
          <ChatWorkspace />
        {:else}
          <TerminalManager />
        {/if}
      </main>
    </div>
    <HotkeyManager />
    {#if showKeyHintsState.current}
      <HotkeyHelp onClose={() => showKeyHints.set(false)} />
    {/if}
    {#if showFuzzyFinder}
      <FuzzyFinder
        onSelect={async (entry) => {
          showFuzzyFinder = false;
          try {
            const project = await command<Project>("load_project", { name: entry.name, repoPath: entry.path });
            await refreshProjectsFromBackend();
            expandedProjects.update(s => { const next = new Set(s); next.add(project.id); return next; });
            focusTarget.set({ type: "project", projectId: project.id });
          } catch (e) {
            showToast(String(e), "error");
          }
        }}
        onClose={() => (showFuzzyFinder = false)}
      />
    {/if}
    {#if issuesModalTarget}
      <IssuesModal
        repoPath={issuesModalTarget.repoPath}
        projectId={issuesModalTarget.projectId}
        onClose={() => { issuesModalTarget = null; }}
        onCreateIssue={handleIssueSubmit}
        onAssignIssue={handleAssignIssue}
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
    {#if screenshotPickerState}
      <SessionPickerModal
        onSelect={(s) => sendScreenshotToSession(s.sessionId, s.projectId)}
        onNewSession={sendScreenshotToNewSession}
        onClose={() => { screenshotPickerState = null; }}
      />
    {/if}
    {#if workspaceModePickerVisibleState.current}
      <WorkspaceModePicker />
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
