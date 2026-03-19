<script lang="ts">
  import { onMount } from "svelte";
  import { fromStore } from "svelte/store";
  import { command } from "$lib/backend";
  import { renderMarkdown } from "$lib/markdown";
  import {
    notesChatThreads,
    activeNotesChatId,
    notesChatSelection,
    focusTarget,
    type NotesChatThread,
    type NotesChatMessage,
    type NotesChatSelection,
    type FocusTarget,
  } from "./stores";

  interface AgentEntry {
    name: string;
    title: string;
  }

  interface Props {
    projectId?: string;
    noteContent?: string;
  }

  let { projectId, noteContent = "" }: Props = $props();

  const threadsState = fromStore(notesChatThreads);
  let threads: NotesChatThread[] = $derived(threadsState.current);
  const activeIdState = fromStore(activeNotesChatId);
  let activeId: string | null = $derived(activeIdState.current);
  const focusTargetState = fromStore(focusTarget);
  let currentFocus: FocusTarget = $derived(focusTargetState.current);
  let isFocused = $derived(currentFocus?.type === "notes-chat");
  const selectionState = fromStore(notesChatSelection);
  let selection: NotesChatSelection | null = $derived(selectionState.current);

  let activeThread = $derived(threads.find((t) => t.id === activeId) ?? null);

  let inputValue = $state("");
  let inputEl: HTMLTextAreaElement | undefined = $state();
  let inputFocused = $state(false);
  let scrollContainer: HTMLDivElement | undefined = $state();
  let loading = $state(false);

  // Agent state
  let agents = $state<AgentEntry[]>([]);
  let selectedAgent = $state<string | null>(null);
  let agentInstructionsCache = $state(new Map<string, string>());

  onMount(async () => {
    if (projectId) {
      try {
        agents = await command<AgentEntry[]>("list_agents", { projectId });
      } catch {
        // silently fail
      }
    }
  });

  // Expose methods for HotkeyManager to call
  export function focusInput() {
    inputFocused = true;
    requestAnimationFrame(() => {
      inputEl?.focus();
    });
  }

  export function createThread() {
    const id = crypto.randomUUID();
    const idx = threads.length + 1;
    const thread: NotesChatThread = {
      id,
      title: `Chat ${idx}`,
      messages: [],
    };
    notesChatThreads.update((ts) => [...ts, thread]);
    activeNotesChatId.set(id);
  }

  export function deleteActiveThread() {
    if (!activeId) return;
    notesChatThreads.update((ts) => {
      const idx = ts.findIndex((t) => t.id === activeId);
      const next = ts.filter((t) => t.id !== activeId);
      if (next.length === 0) {
        activeNotesChatId.set(null);
      } else {
        const newIdx = Math.min(idx, next.length - 1);
        activeNotesChatId.set(next[newIdx].id);
      }
      return next;
    });
  }

  export function navigateThread(direction: 1 | -1) {
    if (threads.length === 0) return;
    const idx = threads.findIndex((t) => t.id === activeId);
    const len = threads.length;
    const next = ((idx + direction) % len + len) % len;
    activeNotesChatId.set(threads[next].id);
  }

  function handleInputKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    }
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      inputFocused = false;
      inputEl?.blur();
    }
  }

  function handleInputFocus() {
    inputFocused = true;
  }

  function handleInputBlur() {
    inputFocused = false;
  }

  async function getAgentInstructions(): Promise<string | null> {
    if (!selectedAgent || !projectId) return null;

    const cached = agentInstructionsCache.get(selectedAgent);
    if (cached) return cached;

    try {
      const content = await command<string>("read_agent_instructions", {
        projectId,
        agentName: selectedAgent,
      });
      agentInstructionsCache.set(selectedAgent, content);
      return content;
    } catch {
      return null;
    }
  }

  async function sendMessage() {
    const text = inputValue.trim();
    if (!text || !activeThread || loading) return;

    const userMsg: NotesChatMessage = { role: "user", content: text };
    notesChatThreads.update((ts) =>
      ts.map((t) =>
        t.id === activeId
          ? { ...t, messages: [...t.messages, userMsg] }
          : t,
      ),
    );
    inputValue = "";
    inputFocused = false;
    inputEl?.blur();
    loading = true;
    scrollToBottom();

    try {
      const agentInstructions = await getAgentInstructions();
      const history = activeThread.messages.map((m) => ({
        role: m.role,
        content: m.content,
      }));

      const response = await command<{ type: string; text: string }>(
        "send_note_ai_chat",
        {
          noteContent,
          selectedText: selection?.selectedText ?? "",
          conversationHistory: history,
          prompt: text,
          agentInstructions,
        },
      );

      const assistantMsg: NotesChatMessage = {
        role: "assistant",
        content: response.text,
      };
      notesChatThreads.update((ts) =>
        ts.map((t) =>
          t.id === activeId
            ? { ...t, messages: [...t.messages, assistantMsg] }
            : t,
        ),
      );
    } catch (error) {
      const errMsg: NotesChatMessage = {
        role: "assistant",
        content: `Error: ${error instanceof Error ? error.message : String(error)}`,
      };
      notesChatThreads.update((ts) =>
        ts.map((t) =>
          t.id === activeId
            ? { ...t, messages: [...t.messages, errMsg] }
            : t,
        ),
      );
    } finally {
      loading = false;
      scrollToBottom();
    }
  }

  function scrollToBottom() {
    requestAnimationFrame(() => {
      if (scrollContainer) {
        scrollContainer.scrollTop = scrollContainer.scrollHeight;
      }
    });
  }

  let selectedAgentTitle = $derived(
    selectedAgent
      ? agents.find((a) => a.name === selectedAgent)?.title ?? selectedAgent
      : null
  );

  let selectionPreview = $derived(
    selection?.selectedText
      ? selection.selectedText.length > 120
        ? selection.selectedText.slice(0, 120) + "..."
        : selection.selectedText
      : null
  );
</script>

<div class="notes-chat-sidebar" class:focused={isFocused}>
  <div class="panel-header">
    <span class="panel-title">Agent Chat</span>
    {#if agents.length > 0}
      <select
        class="agent-select"
        value={selectedAgent ?? ""}
        onchange={(e) => {
          const val = e.currentTarget.value;
          selectedAgent = val || null;
        }}
      >
        <option value="">No agent</option>
        {#each agents as agent}
          <option value={agent.name}>{agent.title || agent.name}</option>
        {/each}
      </select>
    {/if}
    {#if threads.length > 1}
      <span class="thread-indicator">{threads.findIndex((t) => t.id === activeId) + 1}/{threads.length}</span>
    {/if}
  </div>

  {#if selectionPreview}
    <div class="selection-context">
      <pre>{selectionPreview}</pre>
    </div>
  {/if}

  {#if threads.length === 0}
    <div class="empty-state">
      <div class="empty-title">No chats</div>
      <div class="empty-hint">press <kbd>n</kbd> to start one</div>
    </div>
  {:else if activeThread}
    <div class="thread-tabs">
      {#each threads as thread (thread.id)}
        <button
          class="thread-tab"
          class:active={thread.id === activeId}
          onclick={() => activeNotesChatId.set(thread.id)}
        >{thread.title}</button>
      {/each}
    </div>

    <div class="messages" bind:this={scrollContainer}>
      {#if activeThread.messages.length === 0}
        <div class="messages-empty">
          <span class="messages-empty-text">press <kbd>c</kbd> to chat</span>
        </div>
      {:else}
        {#each activeThread.messages as msg}
          <div class="message {msg.role}">
            <span class="message-role">{msg.role === "user" ? "You" : selectedAgentTitle ?? "Agent"}</span>
            <div class="message-content">
              {#if msg.role === "assistant"}
                {@html renderMarkdown(msg.content)}
              {:else}
                {msg.content}
              {/if}
            </div>
          </div>
        {/each}
        {#if loading}
          <div class="message assistant">
            <span class="message-role">{selectedAgentTitle ?? "Agent"}</span>
            <span class="spinner"></span>
          </div>
        {/if}
      {/if}
    </div>

    <div class="input-area">
      <textarea
        bind:this={inputEl}
        bind:value={inputValue}
        placeholder={selectedAgentTitle ? `Ask ${selectedAgentTitle}...` : "Message..."}
        rows="2"
        class:active={inputFocused}
        disabled={loading}
        onfocus={handleInputFocus}
        onblur={handleInputBlur}
        onkeydown={handleInputKeydown}
      ></textarea>
    </div>
  {/if}
</div>

<style>
  .notes-chat-sidebar {
    width: 320px;
    min-width: 320px;
    height: 100%;
    background: var(--bg-surface);
    border-left: 1px solid var(--border-default);
    display: flex;
    flex-direction: column;
    color: var(--text-primary);
    font-size: 13px;
  }

  .notes-chat-sidebar.focused {
    border-left-color: var(--focus-ring);
  }

  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px;
    border-bottom: 1px solid var(--border-default);
    flex-shrink: 0;
    gap: 8px;
  }

  .panel-title {
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-secondary);
  }

  .agent-select {
    flex: 1;
    min-width: 0;
    background: var(--bg-void);
    border: 1px solid var(--border-default);
    border-radius: 4px;
    padding: 3px 6px;
    color: var(--text-primary);
    font-size: 11px;
    font-family: inherit;
    outline: none;
    cursor: pointer;
  }

  .agent-select:focus {
    border-color: var(--text-emphasis);
  }

  .thread-indicator {
    font-size: 11px;
    color: var(--text-secondary);
    font-family: var(--font-mono);
    flex-shrink: 0;
  }

  .selection-context {
    padding: 6px 12px;
    border-bottom: 1px solid var(--border-default);
    max-height: 80px;
    overflow-y: auto;
    flex-shrink: 0;
    background: rgba(137, 180, 250, 0.04);
  }

  .selection-context pre {
    margin: 0;
    font-size: 11px;
    color: var(--text-secondary);
    white-space: pre-wrap;
    word-break: break-word;
    font-family: var(--font-mono);
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    flex: 1;
    gap: 8px;
  }

  .empty-title {
    font-size: 14px;
    font-weight: 500;
    color: var(--text-secondary);
  }

  .empty-hint {
    color: var(--text-secondary);
    font-size: 12px;
  }

  .empty-hint kbd,
  .messages-empty-text kbd {
    background: var(--bg-hover);
    color: var(--text-emphasis);
    padding: 1px 5px;
    border-radius: 3px;
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .thread-tabs {
    display: flex;
    gap: 0;
    border-bottom: 1px solid var(--border-default);
    overflow-x: auto;
    flex-shrink: 0;
  }

  .thread-tab {
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--text-secondary);
    font-size: 12px;
    padding: 6px 12px;
    cursor: pointer;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .thread-tab:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  .thread-tab.active {
    color: var(--text-emphasis);
    border-bottom-color: var(--text-emphasis);
  }

  .messages {
    flex: 1;
    overflow-y: auto;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
  }

  .messages-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: 1;
    color: var(--text-secondary);
    font-size: 12px;
  }

  .message {
    display: flex;
    flex-direction: column;
    gap: 2px;
    line-height: 1.5;
  }

  .message-role {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.3px;
  }

  .message.user .message-role {
    color: var(--text-emphasis);
  }

  .message-content {
    white-space: pre-wrap;
    word-break: break-word;
  }

  .spinner {
    display: inline-block;
    width: 12px;
    height: 12px;
    border: 2px solid var(--bg-active);
    border-top-color: var(--text-emphasis);
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .input-area {
    border-top: 1px solid var(--border-default);
    padding: 10px 12px;
    flex-shrink: 0;
  }

  .input-area textarea {
    width: 100%;
    background: var(--bg-void);
    border: 1px solid var(--border-default);
    border-radius: 4px;
    padding: 8px 10px;
    color: var(--text-primary);
    font-size: 13px;
    font-family: var(--font-sans);
    resize: none;
    outline: none;
    box-sizing: border-box;
  }

  .input-area textarea:focus,
  .input-area textarea.active {
    border-color: var(--text-emphasis);
  }

  .input-area textarea::placeholder {
    color: var(--text-secondary);
  }
</style>
