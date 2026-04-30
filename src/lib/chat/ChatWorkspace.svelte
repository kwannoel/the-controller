<script lang="ts">
  import { onMount } from "svelte";
  import { daemonStore, bootstrap, pingDaemon, loadChats, loadProfiles, loadSessions } from "../daemon/store.svelte";
  import { dispatchHotkeyAction, focusTarget } from "$lib/stores";
  import { showToast } from "$lib/toast";
  import DaemonEmptyState from "./DaemonEmptyState.svelte";
  import ChatView from "./ChatView.svelte";

  let createError = $state<string | null>(null);
  let creatingProjectId = $state<string | null>(null);

  onMount(async () => {
    await bootstrap();
    if (daemonStore.reachable) {
      await loadSessions();
      await loadProfiles();
      await loadChats();
    }
  });

  async function handleRetry() {
    await pingDaemon();
    if (daemonStore.reachable) {
      await loadSessions();
      await loadProfiles();
      await loadChats();
    }
  }

  async function createComposerChat(target: { projectId: string; projectCwd: string }) {
    if (creatingProjectId) {
      daemonStore.newChatTarget = null;
      return;
    }
    if (!daemonStore.client) return;
    creatingProjectId = target.projectId;
    createError = null;
    daemonStore.newChatTarget = null;

    try {
      const chat = await daemonStore.client.createChat({
        project_id: target.projectId,
        title: "New chat",
      });
      daemonStore.chats.set(chat.id, chat);
      if (!daemonStore.chatTranscripts.has(chat.id)) {
        daemonStore.chatTranscripts.set(chat.id, []);
      }
      daemonStore.activeChatId = chat.id;
      daemonStore.activeSessionId = null;
      focusTarget.set({ type: "chat", chatId: chat.id, projectId: chat.project_id });
      setTimeout(() => dispatchHotkeyAction({ type: "focus-chat-input" }), 0);
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      createError = `Failed to create chat: ${message}`;
      showToast(createError, "error");
    } finally {
      creatingProjectId = null;
    }
  }

  $effect(() => {
    const target = daemonStore.newChatTarget;
    if (!target) return;
    void createComposerChat(target);
  });
</script>

{#if !daemonStore.reachable}
  <DaemonEmptyState onRetry={handleRetry} />
{:else if daemonStore.activeChatId}
  <ChatView chatId={daemonStore.activeChatId} />
{:else if daemonStore.activeSessionId}
  <ChatView sessionId={daemonStore.activeSessionId} />
{:else}
  <div class="chat-empty">Select or create a chat.</div>
{/if}

{#if createError}
  <div class="chat-error" role="alert">{createError}</div>
{/if}

<style>
  .chat-empty { padding: 16px; color: var(--text-secondary); }
  .chat-error {
    margin: 8px 12px;
    padding: 6px 8px;
    border: 1px solid var(--status-error);
    border-radius: 4px;
    color: var(--status-error);
    font-size: 12px;
  }
</style>
