<script lang="ts">
  import { daemonStore } from "../daemon/store.svelte";
  import { DaemonHttpError } from "../daemon/client";
  import type { Agent } from "../daemon/types";

  let { projectCwd, onClose }: { projectCwd: string; onClose: () => void } = $props();

  let agent = $state<Agent | "">("");
  let initialPrompt = $state("");
  let error = $state<string | null>(null);
  let busy = $state(false);

  const canSubmit = $derived(agent !== "" && !busy);

  async function submit() {
    if (!agent || !daemonStore.client) return;
    busy = true;
    error = null;
    try {
      const req: any = { agent, cwd: projectCwd };
      if (initialPrompt) req.initial_prompt = initialPrompt;
      const res = await daemonStore.client.createSession(req);
      daemonStore.activeSessionId = res.id;
      onClose();
    } catch (e: any) {
      if ((e && (e.name === "DaemonHttpError" || e instanceof DaemonHttpError)) && e.status === 422) {
        error = "Agent binary not configured on the daemon.";
      } else {
        error = String(e);
      }
    } finally {
      busy = false;
    }
  }
</script>

<div class="overlay" role="dialog">
  <div class="dialog">
    <h3>New chat</h3>
    <label>Agent
      <select bind:value={agent} aria-label="Agent">
        <option value="">Select…</option>
        <option value="claude">Claude Code</option>
        <option value="codex">Codex</option>
      </select>
    </label>
    <label>Initial prompt
      <textarea bind:value={initialPrompt} aria-label="Initial prompt" rows="3"></textarea>
    </label>
    {#if error}<p class="err">{error}</p>{/if}
    <div class="actions">
      <button onclick={onClose}>Cancel</button>
      <button disabled={!canSubmit} onclick={submit}>Create</button>
    </div>
  </div>
</div>

<style>
  .overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.7); display: flex; align-items: center; justify-content: center; z-index: 200; }
  .dialog { background: var(--bg-elevated); padding: 24px; border-radius: 8px; min-width: 360px; display: flex; flex-direction: column; gap: 12px; }
  label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; }
  .err { color: #f38ba8; font-size: 12px; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; }
</style>
