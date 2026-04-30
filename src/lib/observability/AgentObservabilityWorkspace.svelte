<script lang="ts">
  import { onMount } from "svelte";
  import { fromStore } from "svelte/store";
  import {
    bootstrap,
    daemonStore,
    loadAgentTrace,
    loadChatLinks,
    loadChats,
    loadProfiles,
    loadSessions,
  } from "../daemon/store.svelte";
  import { focusTarget } from "../stores";
  import { classifyError } from "../daemon/errors";
  import AgentTraceView from "./AgentTraceView.svelte";

  const focusTargetState = fromStore(focusTarget);

  let loading = $state(false);
  let error: string | null = $state(null);
  let loadGeneration = 0;

  const selectedSessionId = $derived(
    focusTargetState.current?.type === "agent-observe"
      ? focusTargetState.current.sessionId
      : daemonStore.activeSessionId ?? [...daemonStore.sessions.keys()][0] ?? null,
  );
  const selectedSession = $derived(selectedSessionId ? daemonStore.sessions.get(selectedSessionId) ?? null : null);
  const traces = $derived(selectedSessionId ? daemonStore.agentTraces.get(selectedSessionId) ?? [] : []);
  const linkedChatIds = $derived([...new Set(traces.map((trace) => trace.turn.chat_id))]);
  const workspaceLinks = $derived(linkedChatIds.flatMap((chatId) => daemonStore.chatWorkspaceLinks.get(chatId) ?? []));

  onMount(async () => {
    loading = true;
    error = null;
    try {
      if (!daemonStore.client) await bootstrap();
      await Promise.all([loadSessions(), loadProfiles(), loadChats()]);
    } catch (e) {
      error = `Unable to load observability state: ${classifyError(e).message}`;
    } finally {
      loading = false;
    }
  });

  async function loadTraceFor(sessionId: string) {
    const generation = ++loadGeneration;
    loading = true;
    error = null;
    try {
      await loadAgentTrace(sessionId);
      if (generation !== loadGeneration) return;
      const trace = daemonStore.agentTraces.get(sessionId) ?? [];
      const chatIds = [...new Set(trace.map((item) => item.turn.chat_id))];
      await Promise.all(chatIds.map((chatId) => loadChatLinks(chatId).catch(() => undefined)));
    } catch (e) {
      if (generation !== loadGeneration) return;
      error = `Unable to load agent trace: ${classifyError(e).message}`;
    } finally {
      if (generation === loadGeneration) loading = false;
    }
  }

  $effect(() => {
    if (!selectedSessionId || !daemonStore.client) return;
    void loadTraceFor(selectedSessionId);
  });
</script>

<AgentTraceView
  session={selectedSession}
  {traces}
  chats={daemonStore.chats}
  {workspaceLinks}
  {loading}
  {error}
/>
