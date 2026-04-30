<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { daemonStore, loadChatLinks } from "../daemon/store.svelte";
  import { reduceTranscript, emptyTranscript, type TranscriptState } from "../daemon/reducer";
  import { openStream } from "../daemon/stream";
  import { classifyError } from "../daemon/errors";
  import { showToast } from "$lib/toast";
  import type { ChatAgentLink, ChatTranscriptEntry, EventRecord, RouteToken, RouteTokenKind } from "../daemon/types";
  import Transcript from "./Transcript.svelte";
  import ChatInput from "./ChatInput.svelte";
  import ChatSummaryPane, { type SummaryAgent, type SummaryWorkspace } from "./ChatSummaryPane.svelte";

  let { sessionId = null, chatId = null }: { sessionId?: string | null; chatId?: string | null } = $props();
  const session = $derived(sessionId ? daemonStore.sessions.get(sessionId) : null);
  const chat = $derived(chatId ? daemonStore.chats.get(chatId) : null);
  const sessionTranscript = $derived(sessionId ? daemonStore.transcripts.get(sessionId) ?? emptyTranscript() : emptyTranscript());
  const chatEntries = $derived(chatId ? daemonStore.chatTranscripts.get(chatId) ?? [] : []);
  const chatTranscript = $derived(chatId ? transcriptFromChatEntries(chatEntries) : emptyTranscript());
  const summaryAgents = $derived(chatId ? summaryAgentsFromChat(chatId, chatEntries) : []);
  const summaryWorkspaces = $derived(chatId ? summaryWorkspacesFromChat(chatId) : []);
  const hasAssociatedAgent = $derived(summaryAgents.length > 0 || (chatId ? (daemonStore.chatAgentLinks.get(chatId)?.length ?? 0) > 0 : false));

  let handle: { close(): void } | null = null;
  let chatLoadGeneration = 0;

  function transcriptFromChatEntries(entries: ChatTranscriptEntry[]): TranscriptState {
    let transcript = emptyTranscript();
    for (const [index, entry] of entries.entries()) {
      const event = eventFromChatEntry(entry, index + 1);
      transcript = reduceTranscript(transcript, event);
    }
    return transcript;
  }

  function eventFromChatEntry(entry: ChatTranscriptEntry, seq: number): EventRecord {
    if (entry.type === "outbox_event") {
      return { ...entry.event, seq };
    }
    return {
      session_id: "",
      seq,
      channel: "inbox",
      kind: "user_text",
      payload: { text: entry.message.body },
      created_at: entry.message.created_at,
      applied_at: entry.message.created_at,
      chat_id: entry.message.chat_id,
      chat_seq: seq,
      turn_id: null,
    };
  }

  function isRouteTokenSpan(span: unknown): span is Pick<RouteToken, "kind" | "handle"> {
    if (!span || typeof span !== "object") return false;
    const candidate = span as { kind?: unknown; handle?: unknown };
    return (
      (candidate.kind === "reusable" || candidate.kind === "shadow") &&
      typeof candidate.handle === "string"
    );
  }

  function tokenSpansFromEntry(entry: ChatTranscriptEntry): Pick<RouteToken, "kind" | "handle">[] {
    if (entry.type !== "user_message") return [];
    if (!Array.isArray(entry.message.token_spans)) return [];
    return entry.message.token_spans.filter(isRouteTokenSpan);
  }

  function profileForHandle(handle: string) {
    return [...daemonStore.profiles.values()].find((candidate) => candidate.handle === handle) ?? null;
  }

  function summaryAgentForToken(span: Pick<RouteToken, "kind" | "handle">): SummaryAgent {
    const profile = profileForHandle(span.handle);
    return {
      id: profile ? `${profile.id}:${span.kind}` : `${span.kind}:${span.handle}`,
      handle: span.handle,
      name: profile?.name ?? span.handle,
      kind: span.kind as RouteTokenKind,
    };
  }

  function summaryAgentForLink(link: ChatAgentLink): SummaryAgent | null {
    const kind: RouteTokenKind = link.route_type === "shadow" ? "shadow" : "reusable";
    const profile = daemonStore.profiles.get(link.profile_id);
    const tokenHandle = link.token_source?.match(/^[@%]([A-Za-z0-9_-]+)$/)?.[1] ?? null;
    const handle = profile?.handle ?? tokenHandle;
    if (!handle) return null;
    return {
      id: profile ? `${profile.id}:${kind}` : `${kind}:${handle}`,
      handle,
      name: profile?.name ?? handle,
      kind,
    };
  }

  function summaryAgentsFromChat(id: string, entries: ChatTranscriptEntry[]): SummaryAgent[] {
    const agentsById = new Map<string, SummaryAgent>();
    let focusedAgentId: string | null = null;
    for (const entry of entries) {
      for (const span of tokenSpansFromEntry(entry)) {
        const agent = summaryAgentForToken(span);
        if (!agentsById.has(agent.id)) agentsById.set(agent.id, agent);
        if (span.kind === "reusable") focusedAgentId = agent.id;
      }
    }
    for (const link of daemonStore.chatAgentLinks.get(id) ?? []) {
      const agent = summaryAgentForLink(link);
      if (!agent) continue;
      if (!agentsById.has(agent.id)) agentsById.set(agent.id, agent);
      if (link.focused) focusedAgentId = agent.id;
    }
    return [...agentsById.values()].map((agent) => ({
      ...agent,
      focused: agent.id === focusedAgentId,
    }));
  }

  function summaryWorkspacesFromChat(id: string): SummaryWorkspace[] {
    return (daemonStore.chatWorkspaceLinks.get(id) ?? []).map((link) => ({
      id: link.id,
      label: link.label,
      path: link.path,
      focused: link.focused,
    }));
  }

  function chatTranscriptEntryKey(entry: ChatTranscriptEntry): string {
    if (entry.type === "user_message") return `message:${entry.message.id}`;
    const event = entry.event;
    return `event:${event.chat_id ?? ""}:${event.session_id}:${event.seq}:${event.channel}:${event.kind}`;
  }

  function mergeChatTranscriptEntries(
    loaded: ChatTranscriptEntry[],
    current: ChatTranscriptEntry[],
  ): ChatTranscriptEntry[] {
    const seen = new Set<string>();
    const merged: ChatTranscriptEntry[] = [];
    for (const entry of [...loaded, ...current]) {
      const key = chatTranscriptEntryKey(entry);
      if (seen.has(key)) continue;
      seen.add(key);
      merged.push(entry);
    }
    return merged;
  }

  async function loadChatTranscriptFor(nextChatId: string) {
    if (!daemonStore.client) return;
    const generation = ++chatLoadGeneration;
    try {
      const entries = await daemonStore.client.readChatTranscript(nextChatId);
      if (generation !== chatLoadGeneration || chatId !== nextChatId) return;
      const current = daemonStore.chatTranscripts.get(nextChatId) ?? [];
      daemonStore.chatTranscripts.set(nextChatId, mergeChatTranscriptEntries(entries, current));
    } catch (e) {
      if (generation !== chatLoadGeneration || chatId !== nextChatId) return;
      const c = classifyError(e);
      if (c.kind === "not_found") {
        daemonStore.chats.delete(nextChatId);
        if (daemonStore.activeChatId === nextChatId) {
          daemonStore.activeChatId = null;
        }
        showToast("Chat no longer exists.", "error");
      } else {
        showToast(`Failed to load chat: ${c.message}`, "error");
      }
    }
  }

  $effect(() => {
    if (!chatId || !daemonStore.client) return;
    void loadChatTranscriptFor(chatId);
    void loadChatLinks(chatId).catch((e) => {
      const c = classifyError(e);
      showToast(`Failed to load chat links: ${c.message}`, "error");
    });
  });

  onMount(async () => {
    if (!daemonStore.client) return;
    if (!sessionId) return;
    try {
      const events = await daemonStore.client.readEvents(sessionId, 0);
      let t = daemonStore.transcripts.get(sessionId) ?? emptyTranscript();
      for (const e of events) t = reduceTranscript(t, e);
      daemonStore.transcripts.set(sessionId, t);
      handle = openStream(sessionId);
    } catch (e) {
      const c = classifyError(e);
      if (c.kind === "not_found") {
        daemonStore.sessions.delete(sessionId);
        if (daemonStore.activeSessionId === sessionId) {
          daemonStore.activeSessionId = null;
        }
        showToast("Session no longer exists.", "error");
      } else {
        showToast(`Failed to load session: ${c.message}`, "error");
      }
    }
  });

  onDestroy(() => handle?.close());
</script>

{#if chat && chatId}
  <div class="view">
    <header>
      <span class="label">{chat.title}</span>
      <span class="agent">chat</span>
    </header>
    <ChatSummaryPane agents={summaryAgents} workspaces={summaryWorkspaces} />
    <Transcript transcript={chatTranscript} sessionId={chatId} />
    <ChatInput {chatId} status="running" statusState={chatTranscript.statusState} {hasAssociatedAgent} />
  </div>
{:else if session && sessionId}
  <div class="view">
    <header>
      <span class="label">{session.label}</span>
      <span class="agent">{session.agent}</span>
      <span class="status status-{session.status}">{session.status}</span>
    </header>
    <Transcript transcript={sessionTranscript} {sessionId} />
    <ChatInput {sessionId} status={session.status} statusState={sessionTranscript.statusState} />
  </div>
{:else}
  <p class="missing">{chatId ? "Chat not found." : "Session not found."}</p>
{/if}

<style>
  .view { display: flex; flex-direction: column; height: 100%; }
  header { display: flex; gap: 12px; padding: 8px 12px; border-bottom: 1px solid var(--border-default); }
  header .agent, header .status { font-size: 11px; opacity: 0.7; }
  .missing { padding: 16px; color: var(--text-secondary); }
</style>
