<script lang="ts">
  import type { TranscriptState } from "../daemon/reducer";
  import MessageBlock from "./MessageBlock.svelte";
  import AgentMessage from "./AgentMessage.svelte";
  let { transcript }: { transcript: TranscriptState } = $props();
</script>

<div class="scroll">
  {#each transcript.events as e (e.seq)}
    <MessageBlock event={e} />
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
