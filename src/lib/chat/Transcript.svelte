<script lang="ts">
  import type { TranscriptState } from "../daemon/reducer";
  import type { EventRecord } from "../daemon/types";
  import MessageBlock from "./MessageBlock.svelte";
  import AgentMessage from "./AgentMessage.svelte";
  import ToolCallBlock from "./ToolCallBlock.svelte";
  import ToolApprovalBlock from "./ToolApprovalBlock.svelte";

  let { transcript, sessionId }: { transcript: TranscriptState; sessionId: string } = $props();

  const resultsByCallId = $derived.by(() => {
    const map = new Map<string, EventRecord>();
    for (const e of transcript.events) {
      if (e.channel === "outbox" && e.kind === "tool_result") {
        const p = e.payload as { call_id: string };
        map.set(p.call_id, e);
      }
    }
    return map;
  });

  const pendingApproval = $derived(transcript.statusState === "waiting_for_tool_approval");
</script>

<div class="scroll">
  {#each transcript.events as e (e.seq)}
    {#if e.channel === "outbox" && e.kind === "tool_call"}
      {@const callId = (e.payload as { call_id: string }).call_id}
      {@const hasResult = resultsByCallId.has(callId)}
      <ToolCallBlock call={e} result={resultsByCallId.get(callId) ?? null} />
      {#if pendingApproval && !hasResult}
        <ToolApprovalBlock {callId} {sessionId} />
      {/if}
    {:else if e.channel === "outbox" && e.kind === "tool_result"}
      <!-- rendered inside its parent ToolCallBlock -->
    {:else}
      <MessageBlock event={e} />
    {/if}
  {/each}
  {#each [...transcript.inProgressBlocks.entries()] as [id, text] (id)}
    <AgentMessage {text} partial />
  {/each}
</div>

<style>
  .scroll {
    flex: 1;
    overflow-y: auto;
    padding: 12px;
  }
</style>
