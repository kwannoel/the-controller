<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { daemonStore } from "../daemon/store.svelte";
  import { reduceTranscript, emptyTranscript } from "../daemon/reducer";
  import { openStream } from "../daemon/stream";
  import Transcript from "./Transcript.svelte";

  let { sessionId }: { sessionId: string } = $props();
  const session = $derived(daemonStore.sessions.get(sessionId));
  const transcript = $derived(daemonStore.transcripts.get(sessionId) ?? emptyTranscript());

  let handle: { close(): void } | null = null;

  onMount(async () => {
    if (!daemonStore.client) return;
    const events = await daemonStore.client.readEvents(sessionId, 0);
    let t = daemonStore.transcripts.get(sessionId) ?? emptyTranscript();
    for (const e of events) t = reduceTranscript(t, e);
    daemonStore.transcripts.set(sessionId, t);
    handle = openStream(sessionId);
  });

  onDestroy(() => handle?.close());
</script>

{#if session}
  <div class="view">
    <header>
      <span class="label">{session.label}</span>
      <span class="agent">{session.agent}</span>
      <span class="status status-{session.status}">{session.status}</span>
    </header>
    <Transcript {transcript} {sessionId} />
  </div>
{:else}
  <p class="missing">Session not found.</p>
{/if}

<style>
  .view { display: flex; flex-direction: column; height: 100%; }
  header { display: flex; gap: 12px; padding: 8px 12px; border-bottom: 1px solid var(--border-default); }
  header .agent, header .status { font-size: 11px; opacity: 0.7; }
  .missing { padding: 16px; color: var(--text-secondary); }
</style>
