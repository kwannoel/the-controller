<script lang="ts">
  import type { TranscriptState } from "../daemon/reducer";
  let { transcript }: { transcript: TranscriptState } = $props();
</script>

<div class="scroll">
  {#each transcript.events as e (e.seq)}
    {#if e.channel === "outbox" && e.kind === "agent_text"}
      <div class="agent">{(e.payload as any).text}</div>
    {:else if e.channel === "inbox" && e.kind === "user_text"}
      <div class="user">{(e.payload as any).text}</div>
    {/if}
  {/each}
  {#each [...transcript.inProgressBlocks.entries()] as [id, text] (id)}
    <div class="agent partial">{text}</div>
  {/each}
</div>

<style>
  .scroll { flex: 1; overflow-y: auto; padding: 12px; }
  .agent, .user { padding: 6px 10px; margin-bottom: 6px; border-radius: 6px; }
  .agent { background: var(--bg-elevated); }
  .user { background: rgba(137, 180, 250, 0.15); text-align: right; }
  .partial::after { content: " ▋"; }
</style>
