<script lang="ts">
  import { renderMarkdown } from "$lib/markdown";
  let { text, partial = false }: { text: string; partial?: boolean } = $props();
  const html = $derived(renderMarkdown(text));
</script>

<div class="agent" class:partial>
  {#if partial}
    <span class="plain">{text}</span>
  {:else}
    {@html html}
  {/if}
</div>

<style>
  .agent {
    padding: 6px 10px;
    margin-bottom: 6px;
    border-radius: 6px;
    background: var(--bg-elevated);
  }
  .agent.partial::after {
    content: " ▋";
  }
  .plain {
    white-space: pre-wrap;
  }
</style>
