<script lang="ts">
  import { command } from "$lib/backend";
  import { renderMarkdown } from "$lib/markdown";
  import type { AiChatRequest } from "./CodeMirrorNoteEditor.svelte";

  interface ConversationItem {
    role: "user" | "assistant";
    content: string;
    responseType?: "replace" | "info";
  }

  interface Props {
    noteContent: string;
    request: AiChatRequest;
    onReplace?: (text: string, from: number, to: number) => void;
    onDismiss?: () => void;
  }

  let { noteContent, request, onReplace, onDismiss }: Props = $props();

  let inputValue = $state("");
  let conversation = $state<ConversationItem[]>([]);
  let loading = $state(false);
  let scrollContainer: HTMLDivElement | undefined;
  let inputEl: HTMLInputElement | undefined;

  // Track the current selection range (may shift after replacements)
  let currentFrom = $state(request.from);
  let currentTo = $state(request.to);

  // Position the panel near the selection
  let panelStyle = $derived((() => {
    const { coords } = request;
    const left = Math.max(8, coords.left);
    const top = coords.bottom + 8;
    return `left: ${left}px; top: ${top}px;`;
  })());

  // Truncated preview of selected text
  let selectedPreview = $derived(
    request.selectedText.length > 200
      ? request.selectedText.slice(0, 200) + "..."
      : request.selectedText
  );

  $effect(() => {
    inputEl?.focus();
  });

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.stopPropagation();
      onDismiss?.();
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
      const response = await command<{ type: string; text: string }>(
        "send_note_ai_chat",
        {
          noteContent,
          selectedText: request.selectedText,
          conversationHistory: history,
          prompt,
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
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="note-ai-panel" style={panelStyle} data-testid="note-ai-panel" onkeydown={handleKeydown}>
  <div class="selected-preview">
    <pre>{selectedPreview}</pre>
  </div>

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
      placeholder="Ask about selection..."
      disabled={loading}
      data-testid="note-ai-input"
    />
  </form>
</div>

<style>
  .note-ai-panel {
    position: fixed;
    width: 400px;
    max-height: 340px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: 8px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    display: flex;
    flex-direction: column;
    z-index: 100;
    font-size: 13px;
    color: var(--text-primary);
  }

  .selected-preview {
    padding: 8px 12px;
    border-bottom: 1px solid var(--border-default);
    max-height: 60px;
    overflow: hidden;
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
    padding: 8px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
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
    padding: 8px;
  }

  .input-row input {
    width: 100%;
    background: var(--bg-void);
    border: 1px solid var(--border-default);
    border-radius: 4px;
    padding: 6px 10px;
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
