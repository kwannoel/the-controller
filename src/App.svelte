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
  import { appConfig, onboardingComplete, hotkeyAction, showKeyHints, sidebarVisible, taskPanelVisible, focusTarget, projects, type Config, type FocusTarget, type Project } from "./lib/stores";

  let ready = $state(false);
  let needsOnboarding = $state(true);
  let sidebarIsVisible = $state(true);
  let hintsVisible = $state(false);
  let taskPanelIsVisible = $state(false);
  let createIssueTarget: { projectId: string; repoPath: string } | null = $state(null);
  let taskPanelRef: { insertIssue: (issue: any) => void } | undefined = $state();

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
      }
    });
    return unsub;
  });

  function handleIssueCreated(issue: any) {
    createIssueTarget = null;
    taskPanelVisible.set(true);
    setTimeout(() => {
      taskPanelRef?.insertIssue(issue);
    }, 50);
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
        <TaskPanel visible={taskPanelIsVisible} bind:this={taskPanelRef} />
      {/if}
    </div>
    <HotkeyManager />
    {#if hintsVisible}
      <HotkeyHelp onClose={() => showKeyHints.set(false)} />
    {/if}
    {#if createIssueTarget}
      <CreateIssueModal
        repoPath={createIssueTarget.repoPath}
        onCreated={handleIssueCreated}
        onClose={() => { createIssueTarget = null; }}
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
