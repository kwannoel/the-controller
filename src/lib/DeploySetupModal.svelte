<script lang="ts">
  import { onMount } from "svelte";
  import { command } from "$lib/backend";

  interface Props {
    onComplete: () => void;
    onClose: () => void;
  }

  let { onComplete, onClose }: Props = $props();

  let step = $state(1);
  let hetznerKey = $state("");
  let cloudflareKey = $state("");
  let rootDomain = $state("");
  let provisioning = $state(false);
  let error = $state<string | null>(null);
  let inputEl: HTMLInputElement | undefined = $state();

  onMount(() => inputEl?.focus());

  async function handleNext() {
    if (step === 1 && hetznerKey.trim()) {
      step = 2;
      setTimeout(() => inputEl?.focus(), 50);
    } else if (step === 2 && cloudflareKey.trim() && rootDomain.trim()) {
      step = 3;
      await provision();
    }
  }

  async function provision() {
    provisioning = true;
    error = null;
    try {
      await command("save_deploy_credentials", {
        credentials: {
          hetzner_api_key: hetznerKey.trim(),
          cloudflare_api_key: cloudflareKey.trim(),
          cloudflare_zone_id: null,
          root_domain: rootDomain.trim(),
          coolify_url: null,
          coolify_api_key: null,
          server_ip: null,
        },
      });
      onComplete();
    } catch (e) {
      error = String(e);
      provisioning = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") handleNext();
    if (e.key === "Escape") onClose();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="overlay" onclick={onClose} role="dialog" onkeydown={handleKeydown}>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal" onclick={(e) => e.stopPropagation()}>
    <div class="modal-title">Deploy Setup — Step {step} of 3</div>

    {#if step === 1}
      <label class="field-label">Hetzner API Key</label>
      <input
        bind:this={inputEl}
        bind:value={hetznerKey}
        type="password"
        class="field-input"
        placeholder="Enter your Hetzner Cloud API token"
      />
      <p class="hint">Get one from Hetzner Cloud Console → Security → API Tokens</p>
    {:else if step === 2}
      <label class="field-label">Cloudflare API Key</label>
      <input
        bind:this={inputEl}
        bind:value={cloudflareKey}
        type="password"
        class="field-input"
        placeholder="Enter your Cloudflare API token"
      />
      <label class="field-label">Root Domain</label>
      <input
        bind:value={rootDomain}
        type="text"
        class="field-input"
        placeholder="e.g. yourdomain.com"
      />
    {:else if step === 3}
      {#if provisioning}
        <p class="status">Provisioning server...</p>
      {:else if error}
        <p class="error">{error}</p>
      {/if}
    {/if}

    <div class="actions">
      <button class="btn cancel" onclick={onClose}>Cancel</button>
      {#if step < 3}
        <button class="btn primary" onclick={handleNext}>Next</button>
      {/if}
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 120;
  }

  .modal {
    background: #1e1e2e;
    border: 1px solid #313244;
    border-radius: 8px;
    padding: 24px;
    min-width: 400px;
    max-width: 480px;
  }

  .modal-title {
    font-size: 16px;
    font-weight: 600;
    color: #cdd6f4;
    margin-bottom: 20px;
  }

  .field-label {
    display: block;
    font-size: 12px;
    color: #a6adc8;
    margin-bottom: 6px;
    margin-top: 12px;
  }

  .field-input {
    width: 100%;
    padding: 8px 12px;
    background: #11111b;
    border: 1px solid #313244;
    border-radius: 4px;
    color: #cdd6f4;
    font-size: 13px;
    outline: none;
    box-sizing: border-box;
  }

  .field-input:focus {
    border-color: #89b4fa;
  }

  .hint {
    font-size: 11px;
    color: #6c7086;
    margin-top: 6px;
  }

  .status { color: #89b4fa; font-size: 14px; }
  .error { color: #f38ba8; font-size: 13px; }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 20px;
  }

  .btn {
    padding: 6px 16px;
    border-radius: 4px;
    font-size: 13px;
    cursor: pointer;
    border: none;
  }

  .cancel { background: #313244; color: #a6adc8; }
  .primary { background: #89b4fa; color: #1e1e2e; font-weight: 600; }
</style>
