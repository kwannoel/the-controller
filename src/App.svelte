<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import Sidebar from "./lib/Sidebar.svelte";
  import TerminalManager from "./lib/TerminalManager.svelte";
  import Onboarding from "./lib/Onboarding.svelte";
  import Toast from "./lib/Toast.svelte";
  import HotkeyManager from "./lib/HotkeyManager.svelte";
  import StatusBar from "./lib/StatusBar.svelte";
  import { appConfig, onboardingComplete, hotkeyAction, showKeyHints, sidebarVisible, type Config } from "./lib/stores";

  let ready = $state(false);
  let needsOnboarding = $state(true);
  let sidebarIsVisible = $state(true);

  $effect(() => {
    const unsub = sidebarVisible.subscribe((v) => { sidebarIsVisible = v; });
    return unsub;
  });

  $effect(() => {
    const unsub = hotkeyAction.subscribe((action) => {
      if (action?.type === "toggle-help") {
        showKeyHints.update((v) => !v);
      }
    });
    return unsub;
  });

  onMount(async () => {
    try {
      // Clean up stale sessions from previous runs
      await invoke("cleanup_stale_sessions");

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
    </div>
    <HotkeyManager />
    <StatusBar />
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
