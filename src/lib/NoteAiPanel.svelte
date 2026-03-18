<script lang="ts">
  import { onMount } from "svelte";
  import { command } from "$lib/backend";
  import { renderMarkdown } from "$lib/markdown";
  import type { AiChatRequest } from "./CodeMirrorNoteEditor.svelte";

  interface AgentEntry {
    name: string;
    title: string;
  }

  interface ConversationItem {
    role: "user" | "assistant";
    content: string;
    responseType?: "replace" | "info";
  }

  interface Props {
    noteContent: string;
    request: AiChatRequest;
    projectId?: string;
    onReplace?: (text: string, from: number, to: number) => void;
    onDismiss?: () => void;
  }

  let { noteContent, request, projectId, onReplace, onDismiss }: Props = $props();

  let inputValue = $state("");
  let conversation = $state<ConversationItem[]>([]);
  let loading = $state(false);
  let scrollContainer: HTMLDivElement | undefined;
  let inputEl: HTMLInputElement | undefined;

  // Agent state
  let agents = $state<AgentEntry[]>([]);
  let selectedAgent = $state<string | null>(null);
  let agentInstructionsCache = $state(new Map<string, string>());

  // Track the current selection range (may shift after replacements)
  let currentFrom = $state(0);
  let currentTo = $state(0);

  $effect(() => {
    currentFrom = request.from;
    currentTo = request.to;
  });

  // Truncated preview of selected text
  let selectedPreview = $derived(
    request.selectedText.length > 200
      ? request.selectedText.slice(0, 200) + "..."
      : request.selectedText
  );

  onMount(async () => {
    if (projectId) {
      try {
        agents = await command<AgentEntry[]>("list_agents", { projectId });
      } catch {
        // silently fail — agent picker just won't appear
      }
    }
  });

  $effect(() => {
    inputEl?.focus();
  });

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.stopPropagation();
      onDismiss?.();
    }
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

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();

    const prompt = inputValue.trim();
    if (!prompt || loading) return;

    // Build conversation history from all items before this prompt
    const history = conversation.map((item) => ({ role: item.role, content: item.content }));

    conversation.push({ role: "user", content: prompt });
    inputValue = "";
    loading = true;

    try {
      const agentInstructions = await getAgentInstructions();

      const response = await command<{ type: string; text: string }>(
        "send_note_ai_chat",
        {
          noteContent,
          selectedText: request.selectedText,
          conversationHistory: history,
          prompt,
          agentInstructions,
        }
      );

      conversation.push({
        role: "assistant",
        content: response.text,
        responseType: response.type as "replace" | "info",
      });

      if (response.type === "replace") {
        const newTo = currentFrom + response.text.length;
        onReplace?.(response.text, currentFrom, currentTo);
        currentTo = newTo;
      }
    } catch (error) {
      conversation.push({
        role: "assistant",
        content: `Error: ${error instanceof Error ? error.message : String(error)}`,
        responseType: "info",
      });
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
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="note-ai-panel" data-testid="note-ai-panel" onkeydown={handleKeydown}>
  <div class="panel-header">
    <span class="panel-title">Chat</span>
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
    <button class="dismiss-btn" onclick={() => onDismiss?.()} aria-label="Close panel">
      &times;
    </button>
  </div>

  {#if selectedAgentTitle}
    <div class="agent-badge">
      {selectedAgentTitle}
    </div>
  {/if}

  {#if request.selectedText}
    <div class="selected-preview">
      <pre>{selectedPreview}</pre>
    </div>
  {/if}

  <div class="conversation" bind:this={scrollContainer}>
    {#each conversation as item}
      <div class="message {item.role}">
        {#if item.role === "user"}
          <span class="label">You:</span> {item.content}
        {:else}
          <div class="ai-response">
            {#if item.responseType === "replace"}
              <span class="badge replace">replaced</span>
            {/if}
            {@html renderMarkdown(item.content)}
          </div>
        {/if}
      </div>
    {/each}
    {#if loading}
      <div class="message assistant">
        <span class="spinner"></span>
      </div>
    {/if}
  </div>

  <form class="input-row" onsubmit={handleSubmit}>
    <input
      bind:this={inputEl}
      bind:value={inputValue}
      placeholder={selectedAgentTitle
        ? `Ask ${selectedAgentTitle}...`
        : request.selectedText
          ? "Ask about selection..."
          : "Ask about this note..."}
      disabled={loading}
      data-testid="note-ai-input"
    />
  </form>
</div>

<style>
  .note-ai-panel {
    width: 380px;
    min-width: 380px;
    height: 100%;
    background: var(--bg-surface);
    border-left: 1px solid var(--border-default);
    display: flex;
    flex-direction: column;
    font-size: 13px;
    color: var(--text-primary);
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

  .agent-badge {
    padding: 4px 12px;
    font-size: 11px;
    color: var(--text-emphasis);
    background: rgba(137, 180, 250, 0.08);
    border-bottom: 1px solid var(--border-default);
    font-weight: 500;
    flex-shrink: 0;
  }

  .dismiss-btn {
    background: none;
    border: none;
    color: var(--text-secondary);
    font-size: 18px;
    cursor: pointer;
    padding: 0 4px;
    line-height: 1;
    flex-shrink: 0;
  }

  .dismiss-btn:hover {
    color: var(--text-primary);
  }

  .selected-preview {
    padding: 8px 12px;
    border-bottom: 1px solid var(--border-default);
    max-height: 120px;
    overflow-y: auto;
    flex-shrink: 0;
  }

  .selected-preview pre {
    margin: 0;
    font-size: 11px;
    color: var(--text-secondary);
    white-space: pre-wrap;
    word-break: break-word;
    font-family: var(--font-mono);
  }

  .conversation {
    flex: 1;
    overflow-y: auto;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
  }

  .message {
    line-height: 1.5;
  }

  .message.user {
    color: var(--text-emphasis);
  }

  .label {
    font-weight: 600;
  }

  .ai-response {
    color: var(--text-primary);
  }

  .badge {
    display: inline-block;
    font-size: 10px;
    padding: 1px 5px;
    border-radius: 3px;
    margin-bottom: 4px;
    font-weight: 600;
  }

  .badge.replace {
    background: var(--status-idle);
    color: var(--bg-void);
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

  .input-row {
    border-top: 1px solid var(--border-default);
    padding: 12px;
    flex-shrink: 0;
  }

  .input-row input {
    width: 100%;
    background: var(--bg-void);
    border: 1px solid var(--border-default);
    border-radius: 4px;
    padding: 8px 10px;
    color: var(--text-primary);
    font-size: 13px;
    outline: none;
    box-sizing: border-box;
  }

  .input-row input:focus {
    border-color: var(--text-emphasis);
  }

  .input-row input::placeholder {
    color: var(--text-secondary);
  }
</style>
