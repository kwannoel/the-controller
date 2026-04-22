<script lang="ts">
  import type { EventRecord } from "../daemon/types";

  let { call, result }: { call: EventRecord; result: EventRecord | null } = $props();

  let expanded = $state(false);

  const callPayload = $derived(call.payload as { call_id: string; tool: string; input: unknown });
  const resultPayload = $derived(
    result ? (result.payload as { output: unknown; is_error: boolean }) : null,
  );

  function summarize(input: unknown): string {
    if (input && typeof input === "object") {
      const asRec = input as Record<string, unknown>;
      for (const k of ["command", "cmd", "path", "file_path", "query"]) {
        if (typeof asRec[k] === "string") return asRec[k] as string;
      }
    }
    if (typeof input === "string") return input;
    return JSON.stringify(input);
  }

  function stringify(value: unknown): string {
    if (typeof value === "string") return value;
    try {
      return JSON.stringify(value, null, 2);
    } catch {
      return String(value);
    }
  }
</script>

<div class="tool">
  <button class="header" aria-expanded={expanded} onclick={() => (expanded = !expanded)}>
    <span class="chev">{expanded ? "▾" : "▸"}</span>
    <span class="icon">🔧</span>
    <span class="tool-name">{callPayload.tool}</span>
    <span class="summary">{summarize(callPayload.input)}</span>
  </button>
  {#if expanded}
    <div class="body">
      <pre class="input"><code>{stringify(callPayload.input)}</code></pre>
      {#if resultPayload}
        <pre class="result" class:tool-error={resultPayload.is_error}><code
            >{stringify(resultPayload.output)}</code
          ></pre>
      {/if}
    </div>
  {/if}
</div>

<style>
  .tool {
    margin-bottom: 6px;
  }
  .header {
    display: flex;
    gap: 8px;
    align-items: center;
    width: 100%;
    background: transparent;
    border: 0;
    color: inherit;
    padding: 4px 8px;
    border-radius: 4px;
    cursor: pointer;
    text-align: left;
    font-size: 12px;
  }
  .header:hover {
    background: rgba(255, 255, 255, 0.05);
  }
  .chev {
    width: 12px;
    opacity: 0.6;
  }
  .tool-name {
    font-weight: 600;
  }
  .summary {
    opacity: 0.7;
    font-family: var(--font-mono, monospace);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .body {
    padding: 8px 8px 8px 32px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  pre {
    background: var(--bg-elevated);
    padding: 8px;
    border-radius: 4px;
    margin: 0;
    overflow-x: auto;
    font-size: 11px;
  }
  .tool-error {
    border-left: 3px solid var(--status-error);
  }
</style>
