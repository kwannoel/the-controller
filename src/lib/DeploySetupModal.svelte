<script lang="ts">
  import { onMount } from "svelte";
  import { command } from "$lib/backend";

  const HETZNER_KEY_INPUT_ID = "deploy-hetzner-key";
  const CLOUDFLARE_KEY_INPUT_ID = "deploy-cloudflare-key";
  const ROOT_DOMAIN_INPUT_ID = "deploy-root-domain";
  const COOLIFY_URL_INPUT_ID = "deploy-coolify-url";
  const COOLIFY_KEY_INPUT_ID = "deploy-coolify-key";
  const SERVER_IP_INPUT_ID = "deploy-server-ip";

  interface Props {
    onComplete: () => void;
    onClose: () => void;
  }

  let { onComplete, onClose }: Props = $props();

  let step = $state(1);
  let hetznerKey = $state("");
  let cloudflareKey = $state("");
  let rootDomain = $state("");
  let coolifyUrl = $state("");
  let coolifyApiKey = $state("");
  let serverIp = $state("");
  let saving = $state(false);
  let error = $state<string | null>(null);
  let inputEl: HTMLInputElement | undefined = $state();

  onMount(() => inputEl?.focus());

  async function handleNext() {
    if (step === 1 && hetznerKey.trim()) {
      step = 2;
      setTimeout(() => inputEl?.focus(), 50);
    } else if (step === 2 && cloudflareKey.trim() && rootDomain.trim()) {
      step = 3;
      setTimeout(() => inputEl?.focus(), 50);
    } else if (step === 3 && coolifyUrl.trim() && coolifyApiKey.trim() && serverIp.trim()) {
      await provision();
    }
  }

  async function provision() {
    saving = true;
    error = null;
    try {
      await command("save_deploy_credentials", {
        credentials: {
          hetzner_api_key: hetznerKey.trim(),
          cloudflare_api_key: cloudflareKey.trim(),
          cloudflare_zone_id: null,
          root_domain: rootDomain.trim(),
          coolify_url: coolifyUrl.trim(),
          coolify_api_key: coolifyApiKey.trim(),
          server_ip: serverIp.trim(),
        },
      });
      onComplete();
    } catch (e) {
      error = String(e);
      saving = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") handleNext();
    if (e.key === "Escape") onClose();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="overlay"
  onclick={onClose}
  role="dialog"
  onkeydown={handleKeydown}
  tabindex="-1"
  aria-modal="true"
>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal" onclick={(e) => e.stopPropagation()} role="presentation">
    <div class="modal-title">Deploy Setup — Step {step} of 3</div>

    {#if step === 1}
      <label class="field-label" for={HETZNER_KEY_INPUT_ID}>Hetzner API Key</label>
      <input
        id={HETZNER_KEY_INPUT_ID}
        bind:this={inputEl}
        bind:value={hetznerKey}
        type="password"
        class="field-input"
        placeholder="Enter your Hetzner Cloud API token"
      />
      <p class="hint">Get one from Hetzner Cloud Console → Security → API Tokens</p>
    {:else if step === 2}
      <label class="field-label" for={CLOUDFLARE_KEY_INPUT_ID}>Cloudflare API Key</label>
      <input
        id={CLOUDFLARE_KEY_INPUT_ID}
        bind:this={inputEl}
        bind:value={cloudflareKey}
        type="password"
        class="field-input"
        placeholder="Enter your Cloudflare API token"
      />
      <label class="field-label" for={ROOT_DOMAIN_INPUT_ID}>Root Domain</label>
      <input
        id={ROOT_DOMAIN_INPUT_ID}
        bind:value={rootDomain}
        type="text"
        class="field-input"
        placeholder="e.g. yourdomain.com"
      />
    {:else if step === 3}
      <label class="field-label" for={COOLIFY_URL_INPUT_ID}>Coolify URL</label>
      <input
        id={COOLIFY_URL_INPUT_ID}
        bind:this={inputEl}
        bind:value={coolifyUrl}
        type="url"
        class="field-input"
        placeholder="https://coolify.example.com"
      />
      <label class="field-label" for={COOLIFY_KEY_INPUT_ID}>Coolify API Key</label>
      <input
        id={COOLIFY_KEY_INPUT_ID}
        bind:value={coolifyApiKey}
        type="password"
        class="field-input"
        placeholder="Enter your Coolify API token"
      />
      <label class="field-label" for={SERVER_IP_INPUT_ID}>Server IP</label>
      <input
        id={SERVER_IP_INPUT_ID}
        bind:value={serverIp}
        type="text"
        class="field-input"
        placeholder="203.0.113.42"
      />
      <p class="hint">Use the Coolify server URL, a personal API token, and the server's public IP.</p>
      {#if saving}
        <p class="status">Saving deploy configuration...</p>
      {:else if error}
        <p class="error">{error}</p>
      {/if}
    {/if}

    <div class="actions">
      <button class="btn cancel" onclick={onClose}>Cancel</button>
      <button class="btn primary" onclick={handleNext}>{step < 3 ? "Next" : "Finish"}</button>
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
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: 8px;
    padding: 24px;
    min-width: 400px;
    max-width: 480px;
  }

  .modal-title {
    font-size: 16px;
    font-weight: 600;
    color: var(--text-emphasis);
    margin-bottom: 20px;
  }

  .field-label {
    display: block;
    font-size: 12px;
    color: var(--text-secondary);
    margin-bottom: 6px;
    margin-top: 12px;
  }

  .field-input {
    width: 100%;
    padding: 8px 12px;
    background: var(--bg-surface);
    border: 1px solid var(--border-default);
    border-radius: 4px;
    color: var(--text-primary);
    font-size: 13px;
    outline: none;
    box-sizing: border-box;
  }

  .field-input:focus {
    border-color: var(--focus-ring);
  }

  .hint {
    font-size: 11px;
    color: var(--text-tertiary);
    margin-top: 6px;
  }

  .status { color: var(--text-emphasis); font-size: 14px; }
  .error { color: var(--status-error); font-size: 13px; }

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

  .cancel { background: var(--bg-active); color: var(--text-secondary); }
  .primary { background: var(--focus-ring); color: var(--bg-void); font-weight: 600; }
</style>
