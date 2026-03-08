<script lang="ts">
  import { onMount } from "svelte";
  import { fromStore } from "svelte/store";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Sidebar from "./lib/Sidebar.svelte";
  import TerminalManager from "./lib/TerminalManager.svelte";
  import Onboarding from "./lib/Onboarding.svelte";
  import Toast from "./lib/Toast.svelte";
  import HotkeyManager from "./lib/HotkeyManager.svelte";
  import HotkeyHelp from "./lib/HotkeyHelp.svelte";

  import MaintainerPanel from "./lib/MaintainerPanel.svelte";
  import CreateIssueModal from "./lib/CreateIssueModal.svelte";
  import IssuePickerModal from "./lib/IssuePickerModal.svelte";
  import TriagePanel from "./lib/TriagePanel.svelte";
  import { showToast } from "./lib/toast";
  import { appConfig, onboardingComplete, hotkeyAction, showKeyHints, sidebarVisible, maintainerPanelVisible, focusTarget, projects, sessionStatuses, activeSessionId, expandedProjects, dispatchHotkeyAction, focusTerminalSoon, type Config, type GithubIssue, type Project, type SessionStatus, type TriageCategory } from "./lib/stores";
  let ready = $state(false);
  let createIssueTarget: { projectId: string; repoPath: string } | null = $state(null);
  let issuePickerTarget: { projectId: string; repoPath: string; kind?: string; background?: boolean } | null = $state(null);
  let triagePanelOpen: TriageCategory | null = $state(null);

  const sidebarVisibleState = fromStore(sidebarVisible);
  const showKeyHintsState = fromStore(showKeyHints);

  const maintainerPanelVisibleState = fromStore(maintainerPanelVisible);
  const onboardingCompleteState = fromStore(onboardingComplete);
  const projectsState = fromStore(projects);
  const activeSessionIdState = fromStore(activeSessionId);
  const focusTargetState = fromStore(focusTarget);

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
      } else if (action?.type === "toggle-maintainer-panel") {
        maintainerPanelVisible.update(v => !v);
      } else if (action?.type === "toggle-maintainer-enabled") {
        toggleMaintainerEnabled();
      } else if (action?.type === "trigger-maintainer-check") {
        triggerMaintainerCheck();
      } else if (action?.type === "toggle-triage-panel") {
        if (action.category) {
          triagePanelOpen = triagePanelOpen ? null : action.category;
        }
      }
    });
    return unsub;
  });

  async function toggleMaintainerEnabled() {
    const focus = focusTargetState.current;
    if (!focus || (focus.type !== "project" && focus.type !== "session")) return;
    const project = projectsState.current.find((p) => p.id === focus.projectId);
    if (!project) return;
    const newEnabled = !project.maintainer.enabled;
    try {
      await invoke("configure_maintainer", {
        projectId: project.id,
        enabled: newEnabled,
        intervalMinutes: project.maintainer.interval_minutes,
      });
      const result: Project[] = await invoke("list_projects");
      projects.set(result);
      showToast(`Maintainer ${newEnabled ? "enabled" : "disabled"}`, "info");
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function triggerMaintainerCheck() {
    const focus = focusTargetState.current;
    if (!focus || (focus.type !== "project" && focus.type !== "session")) return;
    const project = projectsState.current.find((p) => p.id === focus.projectId);
    if (!project) return;
    try {
      await invoke<any>("trigger_maintainer_check", { projectId: project.id });
      showToast("Maintainer check complete", "info");
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function handleIssueSubmit(title: string, priority: "high" | "low") {
    const repoPath = createIssueTarget!.repoPath;
    createIssueTarget = null; // close modal immediately

    try {
      showToast("Generating issue description...", "info");
      const body = await invoke<string>("generate_issue_body", { title });

      showToast("Creating issue...", "info");
      const issue = await invoke<GithubIssue>("create_github_issue", {
        repoPath,
        title,
        body,
      });

      const label = `priority: ${priority}`;
      invoke("add_github_label", {
        repoPath,
        issueNumber: issue.number,
        label,
        description: priority === "high" ? "Important, should be tackled soon" : "Nice to have, can wait",
        color: priority === "high" ? "F38BA8" : "A6E3A1",
      }).catch((e: unknown) => showToast(`Failed to add priority label: ${e}`, "error"));

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
    dispatchHotkeyAction({ type: "create-session", projectId: target.projectId, kind: target.kind });
  }

  async function activateNewSession(sessionId: string, projectId: string) {
    sessionStatuses.update((m: Map<string, SessionStatus>) => {
      const next = new Map(m);
      next.set(sessionId, "working");
      return next;
    });
    activeSessionId.set(sessionId);
    projects.set(await invoke<Project[]>("list_projects"));
    expandedProjects.update((s: Set<string>) => {
      const next = new Set(s);
      next.add(projectId);
      return next;
    });
  }

  async function createSessionWithIssue(projectId: string, repoPath: string, issue: GithubIssue, kind?: string, background?: boolean) {
    try {
      const sessionId: string = await invoke("create_session", {
        projectId,
        githubIssue: issue,
        kind: kind ?? "claude",
        background: background ?? false,
      });
      // Post comment on the issue (fire and forget)
      invoke("post_github_comment", {
        repoPath,
        issueNumber: issue.number,
        body: `Working on this in session \`${sessionId.substring(0, 8)}\``,
      }).catch((e: unknown) => showToast(`Failed to post comment: ${e}`, "error"));
      // Add in-progress label (fire and forget)
      invoke("add_github_label", {
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
    // Determine project from current focus or active session
    const projectId = focusTargetState.current?.projectId
      ?? projectsState.current.find((p) => p.sessions.some((s) => s.id === activeSessionIdState.current))?.id
      ?? projectsState.current[0]?.id;

    if (!projectId) {
      showToast("No project to create session in", "error");
      return;
    }

    try {
      // 1. Capture screenshot
      showToast(cropped ? "Select area to capture..." : "Capturing screenshot...", "info");
      const screenshotPath: string = await invoke("capture_app_screenshot", { cropped });

      // Open in Preview only when preview is requested
      if (preview) {
        import("@tauri-apps/plugin-opener").then(({ openPath }) => openPath(screenshotPath));
      }

      // 2. Create a new session with initial prompt referencing the screenshot file.
      // Tell Claude to share the path and wait for further instructions.
      const sessionId: string = await invoke("create_session", {
        projectId,
        kind: "claude",
        initialPrompt: `I just took a screenshot of the app. The screenshot is saved at: ${screenshotPath}\nPlease read the screenshot image and share what you see, but wait for further prompts before taking any action.`,
      });

      await activateNewSession(sessionId, projectId);
      focusTerminalSoon();
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  onMount(async () => {
    getCurrentWindow().setTitle(
      `The Controller (${__BUILD_COMMIT__}, ${__BUILD_BRANCH__}, localhost:${__DEV_PORT__})`,
    );

    try {
      // Re-spawn PTY sessions for persisted active sessions
      await invoke("restore_sessions");

      const config = await invoke<Config | null>("check_onboarding");
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
        <TerminalManager />
      </main>

      {#if maintainerPanelVisibleState.current}
        <MaintainerPanel />
      {/if}
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
    {#if triagePanelOpen}
      <TriagePanel category={triagePanelOpen} onClose={() => { triagePanelOpen = null; }} />
    {/if}
  {/if}
{/if}
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
