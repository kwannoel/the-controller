<script lang="ts">
  import { onMount } from "svelte";
  import { daemonStore, bootstrap, pingDaemon, loadSessions } from "../daemon/store.svelte";
  import DaemonEmptyState from "./DaemonEmptyState.svelte";

  onMount(async () => {
    await bootstrap();
    if (daemonStore.reachable) await loadSessions();
  });

  async function handleRetry() {
    await pingDaemon();
    if (daemonStore.reachable) await loadSessions();
  }
</script>

{#if !daemonStore.reachable}
  <DaemonEmptyState onRetry={handleRetry} />
{:else}
  <div class="chat-main">
    <p>Chat mode (placeholder)</p>
  </div>
{/if}

<style>
  .chat-main { padding: 16px; color: var(--text-primary); }
</style>
