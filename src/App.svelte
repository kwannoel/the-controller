<script lang="ts">
  import { onMount } from "svelte";
  import { fromStore } from "svelte/store";
  import { command } from "$lib/backend";
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
  import TriagePanel from "./lib/TriagePanel.svelte";
  import AssignedIssuesPanel from "./lib/AssignedIssuesPanel.svelte";
  import KeystrokeVisualizer from "./lib/KeystrokeVisualizer.svelte";
  import WorkspaceModePicker from "./lib/WorkspaceModePicker.svelte";
  import AgentDashboard from "./lib/AgentDashboard.svelte";
  import NotesEditor from "./lib/NotesEditor.svelte";
  import { refreshProjectsFromBackend } from "./lib/project-listing";
  import { showToast } from "./lib/toast";
  import { appConfig, onboardingComplete, hotkeyAction, showKeyHints, sidebarVisible, workspaceModePickerVisible, workspaceMode, focusTarget, projects, sessionStatuses, activeSessionId, expandedProjects, dispatchHotkeyAction, focusTerminalSoon, selectedSessionProvider, type Config, type GithubIssue, type Project, type SavedPrompt, type SessionStatus, type TriageCategory } from "./lib/stores";
  let ready = $state(false);
  let createIssueTarget: { projectId: string; repoPath: string } | null = $state(null);
  let issuePickerTarget: { projectId: string; repoPath: string; kind?: string; background?: boolean } | null = $state(null);
  let triagePanelOpen: TriageCategory | null = $state(null);
  let assignedIssuesPanelOpen = $state(false);
  let promptPickerTarget: { projectId: string } | null = $state(null);

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
    const project = getTargetProject();

    if (!project) {
      showToast("Select a project before starting a screenshot session", "error");
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
      getCurrentWindow().setTitle(
        `The Controller (${commit}, ${branch}, localhost:${__DEV_PORT__})`,
      );
    } catch {
      // Browser mode — no Tauri window API available
    }
  }

  // Reactively update title when staging state changes
  $effect(() => {
    const stagedProject = projectsState.current.find((p) => p.staged_session);
    if (stagedProject) {
      command<[string, string]>("get_repo_head", { repoPath: stagedProject.repo_path })
        .then(([branch, commit]) => updateWindowTitle(branch, commit))
        .catch(() => {
          // Fallback to staged_session info
          updateWindowTitle(stagedProject.staged_session!.staging_branch, "");
        });
    } else {
      updateWindowTitle(__BUILD_BRANCH__, __BUILD_COMMIT__);
    }
  });

  onMount(async () => {
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
  });
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
        {:else if workspaceModeState.current === "notes"}
          <NotesEditor />
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
    {#if triagePanelOpen}
      <TriagePanel category={triagePanelOpen} onClose={() => { triagePanelOpen = null; }} />
    {/if}
    {#if assignedIssuesPanelOpen}
      <AssignedIssuesPanel onClose={() => { assignedIssuesPanelOpen = false; }} />
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
    background: #11111b;
    overflow: hidden;
  }
  .terminal-area {
    flex: 1;
    overflow: hidden;
  }
</style>
