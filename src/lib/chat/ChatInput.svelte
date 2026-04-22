<script lang="ts">
  import { daemonStore } from "../daemon/store.svelte";
  import { classifyError } from "../daemon/errors";
  import { showToast } from "$lib/toast";
  import type { SessionStatus, StatusState } from "../daemon/types";

  let { sessionId, status, statusState }: {
    sessionId: string;
    status: SessionStatus;
    statusState: StatusState | null;
  } = $props();

  let value = $state("");
  let busy = $state(false);
  let sessionEndedBanner = $state(false);

  const disabled = $derived(status === "ended" || status === "failed" || sessionEndedBanner);
  const canInterrupt = $derived(
    statusState === "working" ||
    statusState === "starting" ||
    statusState === "waiting_for_tool_approval"
  );

  async function sendText() {
    const text = value.trim();
    if (!text || busy || disabled || !daemonStore.client) return;
    busy = true;
    try {
      await daemonStore.client.sendMessage(sessionId, { kind: "user_text", text });
      value = "";
    } catch (e) {
      const c = classifyError(e);
      if (c.kind === "session_ended") {
        sessionEndedBanner = true;
      } else if (
        c.kind === "invalid" ||
        c.kind === "storage" ||
        c.kind === "network" ||
        c.kind === "auth" ||
        c.kind === "not_found"
      ) {
        showToast(`Daemon error: ${c.message}`, "error");
      } else {
        showToast(`Error: ${c.message}`, "error");
      }
    } finally {
      busy = false;
    }
  }

  async function interrupt() {
    if (!canInterrupt || !daemonStore.client) return;
    await daemonStore.client.sendMessage(sessionId, { kind: "interrupt" });
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      void sendText();
      return;
    }
    if (e.key === "Escape") {
      if (canInterrupt) {
        e.preventDefault();
        void interrupt();
      }
    }
  }
</script>

<div class="input">
  {#if sessionEndedBanner}
    <div class="banner" role="alert">Session ended. Start a new one.</div>
  {/if}
  <textarea
    aria-label="Chat input"
    bind:value
    onkeydown={handleKeyDown}
    {disabled}
    placeholder={disabled ? "Session ended." : "Type a message… (⌘⏎ to send, Esc to interrupt)"}
    rows="3"
  ></textarea>
</div>

<style>
  .input { padding: 8px 12px; border-top: 1px solid var(--border-default); }
  .banner {
    margin-bottom: 8px;
    padding: 6px 8px;
    border: 1px solid var(--border-default);
    border-radius: 4px;
    background: var(--bg-elevated);
    color: var(--text-primary);
    font-size: 12px;
  }
  textarea {
    width: 100%; resize: vertical; min-height: 48px;
    background: var(--bg-elevated); color: var(--text-primary);
    border: 1px solid var(--border-default); border-radius: 4px;
    padding: 8px; font-family: inherit; font-size: 13px;
  }
  textarea:disabled { opacity: 0.6; }
</style>
