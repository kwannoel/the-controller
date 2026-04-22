<script lang="ts">
  import type { EventRecord } from "../daemon/types";
  import UserMessage from "./UserMessage.svelte";
  import AgentMessage from "./AgentMessage.svelte";

  let { event }: { event: EventRecord } = $props();
</script>

{#if event.channel === "inbox" && event.kind === "user_text"}
  <UserMessage text={(event.payload as any).text} />
{:else if event.channel === "outbox" && event.kind === "agent_text"}
  <AgentMessage text={(event.payload as any).text} />
{:else if event.channel === "outbox" && event.kind === "agent_thinking"}
  <AgentMessage text={(event.payload as any).text} />
{/if}
