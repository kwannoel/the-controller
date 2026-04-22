<script lang="ts">
  import { onMount } from "svelte";
  import { daemonStore, bootstrap, pingDaemon, loadSessions } from "../daemon/store.svelte";
  import DaemonEmptyState from "./DaemonEmptyState.svelte";
  import NewChatDialog from "./NewChatDialog.svelte";
  import ChatView from "./ChatView.svelte";

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
{:else if daemonStore.activeSessionId}
  <ChatView sessionId={daemonStore.activeSessionId} />
{:else}
  <div class="chat-empty">Select or create a chat.</div>
{/if}

{#if daemonStore.newChatTarget}
  <NewChatDialog projectCwd={daemonStore.newChatTarget.projectCwd} onClose={closeNewChat} />
{/if}

<style>
  .chat-empty { padding: 16px; color: var(--text-secondary); }
</style>
