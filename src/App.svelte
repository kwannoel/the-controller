<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
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
  import { appConfig, onboardingComplete, hotkeyAction, showKeyHints, sidebarVisible, taskPanelVisible, focusTarget, projects, sessionStatuses, activeSessionId, expandedProjects, type Config, type FocusTarget, type Project } from "./lib/stores";

  interface GithubIssue {
    number: number;
    title: string;
    url: string;
    labels: { name: string }[];
  }

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

  interface GithubIssueForSession {
    number: number;
    title: string;
    url: string;
    labels: { name: string }[];
  }

  function handleIssuePicked(issue: GithubIssueForSession) {
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

  async function createSessionWithIssue(projectId: string, repoPath: string, issue: GithubIssueForSession, kind?: string, background?: boolean) {
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

  onMount(async () => {
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
