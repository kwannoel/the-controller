<script lang="ts">
  import { onMount } from "svelte";
  import { fromStore } from "svelte/store";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Sidebar from "./lib/Sidebar.svelte";
  import TerminalManager from "./lib/TerminalManager.svelte";
  import Onboarding from "./lib/Onboarding.svelte";
  import Toast from "./lib/Toast.svelte";
  import HotkeyManager from "./lib/HotkeyManager.svelte";
  import HotkeyHelp from "./lib/HotkeyHelp.svelte";
  import TaskPanel from "./lib/TaskPanel.svelte";
  import CreateIssueModal from "./lib/CreateIssueModal.svelte";
  import IssuePickerModal from "./lib/IssuePickerModal.svelte";
  import { showToast } from "./lib/toast";
  import { appConfig, onboardingComplete, hotkeyAction, showKeyHints, sidebarVisible, taskPanelVisible, focusTarget, projects, sessionStatuses, activeSessionId, expandedProjects, dispatchHotkeyAction, focusTerminalSoon, type Config, type GithubIssue, type Project, type SessionStatus } from "./lib/stores";

  let ready = $state(false);
  let createIssueTarget: { projectId: string; repoPath: string } | null = $state(null);
  let issuePickerTarget: { projectId: string; repoPath: string; kind?: string; background?: boolean } | null = $state(null);

  const sidebarVisibleState = fromStore(sidebarVisible);
  const showKeyHintsState = fromStore(showKeyHints);
  const taskPanelVisibleState = fromStore(taskPanelVisible);
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
        screenshotToNewSession();
      }
    });
    return unsub;
  });

  async function handleIssueSubmit(title: string) {
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

  async function screenshotToNewSession() {
    // Determine project from current focus or active session
    const projectId = focusTargetState.current?.projectId
      ?? projectsState.current.find((p) => p.sessions.some((s) => s.id === activeSessionIdState.current))?.id
      ?? projectsState.current[0]?.id;

    if (!projectId) {
      showToast("No project to create session in", "error");
      return;
    }

    try {
      // 1. Capture screenshot of the app window → clipboard
      showToast("Capturing screenshot...", "info");
      const screenshotPath: string = await invoke("capture_app_screenshot");

      // Open in Preview so user can verify the capture
      const { openPath } = await import("@tauri-apps/plugin-opener");
      await openPath(screenshotPath);

      // 2. Create a new session
      const sessionId: string = await invoke("create_session", {
        projectId,
        kind: "claude",
      });

      // 3. Register idle listener IMMEDIATELY after session creation.
      // create_session is synchronous (blocks main thread) while Claude Code
      // starts in tmux — if worktree creation is slow, Claude may already be
      // idle by the time JS resumes. Registering the listener before any other
      // async work ensures we catch events queued during the blocked period.
      let done = false;
      let timeoutId: ReturnType<typeof setTimeout> | null = null;
      const unlisten = await listen<string>(`session-status-hook:${sessionId}`, (event) => {
        if (event.payload !== "idle" || done) return;
        done = true;
        if (timeoutId) clearTimeout(timeoutId);
        unlisten();
        // Send bracket paste to trigger Claude Code's clipboard image reader
        invoke("write_to_pty", { sessionId, data: "\x1b[200~\x1b[201~" });
      });

      timeoutId = setTimeout(() => {
        if (done) return;
        done = true;
        unlisten();
      }, 30000);

      await activateNewSession(sessionId, projectId);

      // 4. Focus the terminal
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
      {#if taskPanelVisibleState.current}
        <TaskPanel />
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
