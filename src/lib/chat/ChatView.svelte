<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { daemonStore } from "../daemon/store.svelte";
  import { reduceTranscript, emptyTranscript, type TranscriptState } from "../daemon/reducer";
  import { openStream } from "../daemon/stream";
  import { classifyError } from "../daemon/errors";
  import { showToast } from "$lib/toast";
  import type { ChatTranscriptEntry, EventRecord } from "../daemon/types";
  import Transcript from "./Transcript.svelte";
  import ChatInput from "./ChatInput.svelte";

  let { sessionId = null, chatId = null }: { sessionId?: string | null; chatId?: string | null } = $props();
  const session = $derived(sessionId ? daemonStore.sessions.get(sessionId) : null);
  const chat = $derived(chatId ? daemonStore.chats.get(chatId) : null);
  const sessionTranscript = $derived(sessionId ? daemonStore.transcripts.get(sessionId) ?? emptyTranscript() : emptyTranscript());
  const chatTranscript = $derived(chatId ? transcriptFromChatEntries(daemonStore.chatTranscripts.get(chatId) ?? []) : emptyTranscript());

  let handle: { close(): void } | null = null;

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

  onMount(async () => {
    if (!daemonStore.client) return;
    if (chatId) {
      try {
        const entries = await daemonStore.client.readChatTranscript(chatId);
        daemonStore.chatTranscripts.set(chatId, entries);
      } catch (e) {
        const c = classifyError(e);
        if (c.kind === "not_found") {
          daemonStore.chats.delete(chatId);
          if (daemonStore.activeChatId === chatId) {
            daemonStore.activeChatId = null;
          }
          showToast("Chat no longer exists.", "error");
        } else {
          showToast(`Failed to load chat: ${c.message}`, "error");
        }
      }
      return;
    }
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
    <Transcript transcript={chatTranscript} sessionId={chatId} />
    <ChatInput {chatId} status="running" statusState={chatTranscript.statusState} />
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
