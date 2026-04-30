<script lang="ts">
  import { tick } from "svelte";
  import { daemonStore } from "../daemon/store.svelte";
  import { classifyError } from "../daemon/errors";
  import { showToast } from "$lib/toast";
  import { focusTarget, hotkeyAction } from "$lib/stores";
  import type { AgentProfile, RouteToken, SessionStatus, StatusState } from "../daemon/types";
  import {
    extractRouteTokenQuery,
    insertRouteToken,
    reconcileRouteTokens,
    routeTokenForProfile,
    type RouteTokenQuery,
  } from "./chat-routing";
  import AgentTokenMenu, { type AgentTokenSelection } from "./AgentTokenMenu.svelte";

  let { sessionId = null, chatId = null, status, statusState, hasAssociatedAgent = false }: {
    sessionId?: string | null;
    chatId?: string | null;
    status: SessionStatus;
    statusState: StatusState | null;
    hasAssociatedAgent?: boolean;
  } = $props();

  let value = $state("");
  let busy = $state(false);
  let sessionEndedBanner = $state(false);
  let textareaEl: HTMLTextAreaElement | undefined = $state();
  let routeTokens = $state<RouteToken[]>([]);
  let tokenQuery = $state<RouteTokenQuery | null>(null);
  let menuActiveIndex = $state(0);
  let localError = $state<string | null>(null);
  let draftIdempotency = $state<{ signature: string; id: string } | null>(null);

  const disabled = $derived(status === "ended" || status === "failed" || sessionEndedBanner);
  const activeProfiles = $derived(
    [...(daemonStore.profiles?.values() ?? [])].filter((profile: AgentProfile) => profile.archived_at === null)
  );
  const suggestedProfiles = $derived.by(() => {
    if (!tokenQuery) return [];
    const needle = tokenQuery.query.trim().toLowerCase();
    return activeProfiles
      .filter((profile) => (
        !needle ||
        profile.handle.toLowerCase().includes(needle) ||
        profile.name.toLowerCase().includes(needle)
      ))
      .sort((a, b) => a.handle.localeCompare(b.handle));
  });
  const canInterrupt = $derived(
    statusState === "working" ||
    statusState === "starting" ||
    statusState === "waiting_for_tool_approval"
  );

  async function sendText() {
    const text = value.trim();
    if (!text || busy || disabled || !daemonStore.client) return;
    const trimStartOffset = value.length - value.trimStart().length;
    const trimEnd = trimStartOffset + text.length;
    const tokensToSend = chatId
      ? reconcileRouteTokens(value, routeTokens).flatMap((token) => {
          if (token.start < trimStartOffset || token.end > trimEnd) return [];
          return [{ ...token, start: token.start - trimStartOffset, end: token.end - trimStartOffset }];
        })
      : [];
    if (chatId && tokensToSend.length === 0 && !hasAssociatedAgent) {
      localError = "Select an agent with @ or % before sending.";
      return;
    }
    const idempotency = chatId ? idempotencyForDraft(text, tokensToSend) : null;
    busy = true;
    try {
      if (chatId) {
        const res = await daemonStore.client.sendChatMessage(chatId, {
          body: text,
          tokens: tokensToSend,
          idempotency_id: idempotency!,
        });
        const prev = daemonStore.chatTranscripts.get(chatId) ?? [];
        daemonStore.chatTranscripts.set(chatId, [...prev, { type: "user_message", message: res.message }]);
        routeTokens = [];
        tokenQuery = null;
        localError = null;
        draftIdempotency = null;
      } else if (sessionId) {
        await daemonStore.client.sendMessage(sessionId, { kind: "user_text", text });
      }
      value = "";
    } catch (e) {
      const c = classifyError(e);
      if (c.kind === "session_ended") {
        sessionEndedBanner = true;
      } else if (
        c.kind === "invalid" ||
        c.kind === "storage" ||
        c.kind === "network" ||
        c.kind === "auth" ||
        c.kind === "not_found"
      ) {
        showToast(`Daemon error: ${c.message}`, "error");
      } else {
        showToast(`Error: ${c.message}`, "error");
      }
    } finally {
      busy = false;
    }
  }

  function idempotencyForDraft(body: string, tokens: RouteToken[]) {
    const signature = JSON.stringify({ chatId, body, tokens });
    if (draftIdempotency?.signature === signature) return draftIdempotency.id;
    const next = { signature, id: crypto.randomUUID() };
    draftIdempotency = next;
    return next.id;
  }

  async function interrupt() {
    if (!canInterrupt || !daemonStore.client || !sessionId) return;
    await daemonStore.client.sendMessage(sessionId, { kind: "interrupt" });
  }

  function focusActiveChatRow() {
    if (!chatId) return false;
    const chat = daemonStore.chats.get(chatId);
    if (!chat) return false;
    focusTarget.set({ type: "chat", chatId: chat.id, projectId: chat.project_id });
    return true;
  }

  function updateTokenQuery(text: string, cursor: number) {
    const nextQuery = chatId ? extractRouteTokenQuery(text, cursor) : null;
    if (nextQuery?.kind !== tokenQuery?.kind || nextQuery?.query !== tokenQuery?.query) {
      menuActiveIndex = 0;
    }
    tokenQuery = nextQuery;
  }

  function handleInput(e: Event) {
    const target = e.currentTarget as HTMLTextAreaElement;
    value = target.value;
    routeTokens = reconcileRouteTokens(target.value, routeTokens);
    localError = null;
    updateTokenQuery(target.value, target.selectionStart);
  }

  function handleSelectToken(selection: AgentTokenSelection) {
    if (!tokenQuery) return;
    const inserted = insertRouteToken(value, tokenQuery, selection.handle);
    const nextToken = routeTokenForProfile(
      { handle: selection.handle },
      selection.kind,
      inserted.token.start,
      inserted.token.end,
    );
    value = inserted.text;
    routeTokens = [...routeTokens, nextToken];
    tokenQuery = null;
    localError = null;
    void tick().then(() => {
      textareaEl?.focus();
      textareaEl?.setSelectionRange(inserted.cursor, inserted.cursor);
    });
  }

  function selectActiveSuggestion() {
    const profile = suggestedProfiles[menuActiveIndex];
    if (!profile || !tokenQuery) return;
    handleSelectToken({ kind: tokenQuery.kind, profileId: profile.id, handle: profile.handle });
  }

  function moveActiveSuggestion(delta: number) {
    if (suggestedProfiles.length === 0) return;
    menuActiveIndex = (menuActiveIndex + delta + suggestedProfiles.length) % suggestedProfiles.length;
  }

  $effect(() => {
    const unsub = hotkeyAction.subscribe((action) => {
      if (action?.type === "focus-chat-input" && textareaEl && !disabled) {
        textareaEl.focus();
      }
    });
    return unsub;
  });

  function handleKeyDown(e: KeyboardEvent) {
    if (tokenQuery && suggestedProfiles.length > 0 && !e.metaKey && !e.ctrlKey) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        moveActiveSuggestion(1);
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        moveActiveSuggestion(-1);
        return;
      }
      if (e.key === "Enter") {
        e.preventDefault();
        selectActiveSuggestion();
        return;
      }
    }
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      void sendText();
      return;
    }
    if (e.key === "Escape") {
      if (e.shiftKey) {
        if (!canInterrupt) return;
        e.preventDefault();
        void interrupt();
      } else {
        if (tokenQuery) {
          e.preventDefault();
          tokenQuery = null;
          return;
        }
        e.preventDefault();
        if (!focusActiveChatRow()) {
          (e.currentTarget as HTMLTextAreaElement).blur();
        }
      }
    }
  }
</script>

<div class="input">
  {#if sessionEndedBanner}
    <div class="banner" role="alert">Session ended. Start a new one.</div>
  {/if}
  {#if localError}
    <div class="banner" role="alert">{localError}</div>
  {/if}
  {#if tokenQuery}
    <AgentTokenMenu
      kind={tokenQuery.kind}
      query={tokenQuery.query}
      profiles={activeProfiles}
      activeIndex={menuActiveIndex}
      onSelect={handleSelectToken}
      onActiveIndexChange={(index) => menuActiveIndex = index}
      onClose={() => tokenQuery = null}
    />
  {/if}
  <textarea
    aria-label="Chat input"
    bind:this={textareaEl}
    value={value}
    oninput={handleInput}
    onkeyup={(e) => updateTokenQuery(value, (e.currentTarget as HTMLTextAreaElement).selectionStart)}
    onclick={(e) => updateTokenQuery(value, (e.currentTarget as HTMLTextAreaElement).selectionStart)}
    onkeydown={handleKeyDown}
    {disabled}
    placeholder={disabled ? "Session ended." : "Type a message… (⌘⏎ to send, ⇧Esc to interrupt)"}
    rows="3"
  ></textarea>
</div>

<style>
  .input { position: relative; padding: 8px 12px; border-top: 1px solid var(--border-default); }
  .banner {
    margin-bottom: 8px;
    padding: 6px 8px;
    border: 1px solid var(--border-default);
    border-radius: 4px;
    background: var(--bg-elevated);
    color: var(--text-primary);
    font-size: 12px;
  }
  textarea {
    width: 100%; resize: vertical; min-height: 48px;
    background: var(--bg-elevated); color: var(--text-primary);
    border: 1px solid var(--border-default); border-radius: 4px;
    padding: 8px; font-family: inherit; font-size: 13px;
  }
  textarea:disabled { opacity: 0.6; }
</style>
