<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
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
  import { appConfig, onboardingComplete, hotkeyAction, showKeyHints, sidebarVisible, taskPanelVisible, focusTarget, projects, sessionStatuses, activeSessionId, expandedProjects, type Config, type FocusTarget, type GithubIssue, type Project } from "./lib/stores";

  let ready = $state(false);
  let needsOnboarding = $state(true);
  let sidebarIsVisible = $state(true);
  let hintsVisible = $state(false);
  let taskPanelIsVisible = $state(false);
  let createIssueTarget: { projectId: string; repoPath: string } | null = $state(null);
  let issuePickerTarget: { projectId: string; repoPath: string; kind?: string; background?: boolean } | null = $state(null);


  $effect(() => {
    const unsub = sidebarVisible.subscribe((v) => { sidebarIsVisible = v; });
    return unsub;
  });

  $effect(() => {
    const unsub = showKeyHints.subscribe((v) => { hintsVisible = v; });
    return unsub;
  });

  $effect(() => {
    const unsub = taskPanelVisible.subscribe((v) => { taskPanelIsVisible = v; });
    return unsub;
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
    hotkeyAction.set({ type: "create-session", projectId: target.projectId, kind: target.kind });
    setTimeout(() => hotkeyAction.set(null), 0);
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

      sessionStatuses.update((m: Map<string, string>) => {
        const next = new Map(m);
        next.set(sessionId, "working");
        return next;
      });
      activeSessionId.set(sessionId);
      const result = await invoke<Project[]>("list_projects");
      projects.set(result);
      expandedProjects.update((s: Set<string>) => { const next = new Set(s); next.add(projectId); return next; });
      setTimeout(() => {
        hotkeyAction.set({ type: "focus-terminal" });
        setTimeout(() => hotkeyAction.set(null), 0);
      }, 50);
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function screenshotToNewSession() {
    // Determine project from current focus or active session
    let projectList: Project[] = [];
    projects.subscribe((v) => { projectList = v; })();
    let active: string | null = null;
    activeSessionId.subscribe((v) => { active = v; })();
    let focus: FocusTarget = null;
    focusTarget.subscribe((v) => { focus = v; })();

    const projectId = focus?.projectId
      ?? projectList.find((p) => p.sessions.some((s) => s.id === active))?.id
      ?? projectList[0]?.id;

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

      sessionStatuses.update((m: Map<string, string>) => {
        const next = new Map(m);
        next.set(sessionId, "working");
        return next;
      });
      activeSessionId.set(sessionId);
      const result = await invoke<Project[]>("list_projects");
      projects.set(result);
      expandedProjects.update((s: Set<string>) => {
        const next = new Set(s);
        next.add(projectId);
        return next;
      });

      // 3. Focus the terminal
      setTimeout(() => {
        hotkeyAction.set({ type: "focus-terminal" });
        setTimeout(() => hotkeyAction.set(null), 0);
      }, 50);

      // 4. When session becomes idle (Claude Code ready), auto-paste the screenshot
      const unsubStatus = sessionStatuses.subscribe((statuses) => {
        if (statuses.get(sessionId) === "idle") {
          unsubStatus();
          // Send bracket paste to trigger Claude Code's clipboard image reader
          invoke("write_to_pty", { sessionId, data: "\x1b[200~\x1b[201~" });
        }
      });

      // Timeout: clean up subscription after 30s if idle never fires
      setTimeout(() => {
        unsubStatus();
      }, 30000);
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
        needsOnboarding = false;
      }
    } catch (e) {
      // Config check failed, show onboarding
    }
    ready = true;
  });

  // Listen for onboarding completion
  $effect(() => {
    const unsub = onboardingComplete.subscribe((complete) => {
      if (complete) needsOnboarding = false;
    });
    return unsub;
  });
</script>

{#if ready}
  {#if needsOnboarding}
    <Onboarding />
  {:else}
    <div class="app-layout">
      {#if sidebarIsVisible}
        <Sidebar />
      {/if}
      <main class="terminal-area">
        <TerminalManager />
      </main>
      {#if taskPanelIsVisible}
        <TaskPanel visible={taskPanelIsVisible} />
      {/if}
    </div>
    <HotkeyManager />
    {#if hintsVisible}
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
