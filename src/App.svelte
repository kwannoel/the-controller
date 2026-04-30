<script lang="ts">
  import { onMount } from "svelte";
  import { fromStore } from "svelte/store";
  import { command, listen } from "$lib/backend";
  import Sidebar from "./lib/Sidebar.svelte";
  import Onboarding from "./lib/Onboarding.svelte";
  import Toast from "./lib/Toast.svelte";
  import HotkeyManager from "./lib/HotkeyManager.svelte";
  import HotkeyHelp from "./lib/HotkeyHelp.svelte";
  import SecureEnvModal from "./lib/SecureEnvModal.svelte";
  import FuzzyFinder from "./lib/FuzzyFinder.svelte";
  import KeystrokeVisualizer from "./lib/KeystrokeVisualizer.svelte";
  import WorkspaceModePicker from "./lib/WorkspaceModePicker.svelte";
  import AgentDashboard from "./lib/AgentDashboard.svelte";
  import AgentCreationWorkspace from "./lib/agents/AgentCreationWorkspace.svelte";
  import KanbanBoard from "./lib/KanbanBoard.svelte";
  import ChatWorkspace from "./lib/chat/ChatWorkspace.svelte";
  import { refreshProjectsFromBackend } from "./lib/project-listing";
  import { showToast } from "./lib/toast";
  import {
    appConfig,
    onboardingComplete,
    hotkeyAction,
    showKeyHints,
    sidebarVisible,
    workspaceModePickerVisible,
    workspaceMode,
    focusTarget,
    projects,
    expandedProjects,
    type Config,
    type Project,
  } from "./lib/stores";

  let ready = $state(false);
  let secureEnvRequest: { requestId: string; projectId: string; projectName: string; key: string } | null = $state(null);
  let showFuzzyFinder = $state(false);

  const sidebarVisibleState = fromStore(sidebarVisible);
  const showKeyHintsState = fromStore(showKeyHints);
  const workspaceModePickerVisibleState = fromStore(workspaceModePickerVisible);
  const workspaceModeState = fromStore(workspaceMode);
  const onboardingCompleteState = fromStore(onboardingComplete);
  const projectsState = fromStore(projects);
  const focusTargetState = fromStore(focusTarget);

  $effect(() => {
    const unsub = hotkeyAction.subscribe((action) => {
      if (action?.type === "open-fuzzy-finder") {
        showFuzzyFinder = true;
      } else if (action?.type === "toggle-help") {
        showKeyHints.update((v) => !v);
      } else if (action?.type === "toggle-maintainer-enabled") {
        toggleMaintainerEnabled();
      } else if (action?.type === "toggle-auto-worker-enabled") {
        toggleAutoWorkerEnabled();
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
    const project = getTargetProject();
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

  function updateWindowTitle(branch: string, commit: string) {
    const parts = [commit, branch, `localhost:${__DEV_PORT__}`];
    document.title = `The Controller (${parts.join(", ")})`;
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
        const config = await command<Config | null>("check_onboarding");
        if (config) {
          appConfig.set(config);
          onboardingComplete.set(true);
        }
      } catch (e) {
        // Config check failed, show onboarding.
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
        {:else if workspaceModeState.current === "agent-create"}
          <AgentCreationWorkspace />
        {:else if workspaceModeState.current === "agent-observe"}
          <section class="workspace-placeholder" aria-label="Agent Observe">
            <div>
              <p>Agent Observe</p>
              <span>Observability workspace is queued for the next agent architecture task.</span>
            </div>
          </section>
        {:else if workspaceModeState.current === "kanban"}
          <KanbanBoard />
        {:else}
          <ChatWorkspace />
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
    {#if secureEnvRequest}
      <SecureEnvModal
        projectName={secureEnvRequest.projectName}
        envKey={secureEnvRequest.key}
        onSubmit={submitSecureEnvValue}
        onClose={cancelSecureEnvRequest}
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
  .workspace-placeholder {
    height: 100%;
    display: grid;
    place-items: center;
    background: var(--bg-void);
    color: var(--text-primary);
  }
  .workspace-placeholder div {
    display: grid;
    gap: 8px;
    border: 1px solid var(--border-default);
    border-radius: 8px;
    background: var(--bg-base);
    padding: 24px;
  }
  .workspace-placeholder p {
    font: 600 16px var(--font-mono);
  }
  .workspace-placeholder span {
    color: var(--text-secondary);
    font-size: 13px;
  }
</style>
