<script lang="ts">
  import { onMount } from "svelte";
  import { daemonStore, bootstrap, pingDaemon, loadSessions } from "../daemon/store.svelte";
  import DaemonEmptyState from "./DaemonEmptyState.svelte";
  import NewChatDialog from "./NewChatDialog.svelte";

  onMount(async () => {
    await bootstrap();
    if (daemonStore.reachable) await loadSessions();
  });

  async function handleRetry() {
    await pingDaemon();
    if (daemonStore.reachable) await loadSessions();
  }

  function closeNewChat() {
    daemonStore.newChatTarget = null;
  }
</script>

{#if !daemonStore.reachable}
  <DaemonEmptyState onRetry={handleRetry} />
{:else}
  <div class="chat-main">
    <p>Chat mode (placeholder)</p>
  </div>
{/if}

{#if daemonStore.newChatTarget}
  <NewChatDialog projectCwd={daemonStore.newChatTarget.projectCwd} onClose={closeNewChat} />
{/if}

<style>
  .chat-main { padding: 16px; color: var(--text-primary); }
</style>
