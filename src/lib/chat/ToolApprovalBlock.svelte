<script lang="ts">
  import { daemonStore } from "../daemon/store.svelte";

  let { callId, sessionId }: { callId: string; sessionId: string } = $props();

  let busy = $state(false);
  let denyMode = $state(false);
  let reason = $state("");

  async function send(approved: boolean, reasonText?: string) {
    if (!daemonStore.client || busy) return;
    busy = true;
    try {
      const body: {
        kind: "tool_approval";
        call_id: string;
        approved: boolean;
        reason?: string;
      } = { kind: "tool_approval", call_id: callId, approved };
      if (reasonText !== undefined && reasonText !== "") body.reason = reasonText;
      await daemonStore.client.sendMessage(sessionId, body);
      denyMode = false;
      reason = "";
    } finally {
      busy = false;
    }
  }
</script>

<div class="approval">
  {#if !denyMode}
    <button disabled={busy} onclick={() => send(true)}>Approve</button>
    <button disabled={busy} onclick={() => send(false)}>Deny</button>
    <button disabled={busy} onclick={() => (denyMode = true)}>Deny with reason</button>
  {:else}
    <textarea
      aria-label="Deny reason"
      bind:value={reason}
      rows="2"
      placeholder="Reason for denying…"
    ></textarea>
    <button disabled={busy} onclick={() => send(false, reason)}>Submit</button>
    <button
      disabled={busy}
      onclick={() => {
        denyMode = false;
        reason = "";
      }}>Cancel</button
    >
  {/if}
</div>

<style>
  .approval {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
    padding: 8px;
    margin-left: 32px;
    margin-bottom: 8px;
    background: rgba(249, 226, 175, 0.08);
    border-left: 2px solid #f9e2af;
    border-radius: 4px;
  }
  button {
    padding: 4px 10px;
    font-size: 12px;
  }
  textarea {
    flex: 1 1 200px;
    font-size: 12px;
  }
</style>
