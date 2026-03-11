<script lang="ts">
  import { onMount } from "svelte";

  interface Props {
    projectName: string;
    envKey: string;
    onSubmit: (value: string) => void;
    onClose: () => void;
  }

  let { projectName, envKey, onSubmit, onClose }: Props = $props();

  let value = $state("");
  let reveal = $state(false);
  let inputEl: HTMLInputElement | undefined = $state();

  onMount(() => {
    inputEl?.focus();
  });

  function submit() {
    if (!value) return;
    onSubmit(value);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    } else if (e.key === "Enter") {
      e.preventDefault();
      submit();
    }
  }
</script>

<div class="overlay" onclick={onClose} onkeydown={handleKeydown} role="dialog" tabindex="0">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal" onclick={(e) => e.stopPropagation()} role="presentation">
    <div class="modal-header">Secure Env Variable</div>
    <div class="meta">
      <div class="meta-row">
        <span class="meta-label">Project</span>
        <span class="meta-value">{projectName}</span>
      </div>
      <div class="meta-row">
        <span class="meta-label">Key</span>
        <span class="meta-value code">{envKey}</span>
      </div>
    </div>
    <label class="label" for="secure-env-value">Secret value</label>
    <div class="input-row">
      <input
        id="secure-env-value"
        bind:this={inputEl}
        bind:value={value}
        class="input"
        type={reveal ? "text" : "password"}
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
      />
      <button class="btn-toggle" type="button" onclick={() => (reveal = !reveal)}>
        {reveal ? "Hide" : "Reveal"}
      </button>
    </div>
    <div class="actions">
      <button class="btn-cancel" type="button" onclick={onClose}>Cancel</button>
      <button class="btn-primary" type="button" onclick={submit} disabled={!value}>Save</button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(16px);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 18vh;
    z-index: 120;
    outline: none;
  }
  .modal {
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: 8px;
    width: 420px;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
  }
  .modal-header {
    font-size: 16px;
    font-weight: 600;
    color: var(--text-emphasis);
  }
  .meta {
    display: grid;
    gap: 8px;
    background: var(--bg-base);
    border: 1px solid var(--border-default);
    border-radius: 6px;
    padding: 12px;
  }
  .meta-row {
    display: flex;
    justify-content: space-between;
    gap: 16px;
  }
  .meta-label {
    color: var(--text-secondary);
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .meta-value {
    color: var(--text-primary);
    font-size: 13px;
  }
  .meta-value.code {
    font-family: var(--font-mono);
  }
  .label {
    color: var(--text-primary);
    font-size: 13px;
  }
  .input-row {
    display: flex;
    gap: 8px;
  }
  .input {
    flex: 1;
    background: var(--bg-void);
    color: var(--text-primary);
    border: 1px solid var(--border-default);
    padding: 10px 12px;
    border-radius: 6px;
    font-size: 14px;
    outline: none;
  }
  .input:focus {
    border-color: var(--text-emphasis);
  }
  .btn-toggle,
  .btn-cancel,
  .btn-primary {
    border: none;
    border-radius: 6px;
    font-size: 13px;
    cursor: pointer;
  }
  .btn-toggle,
  .btn-cancel {
    background: var(--bg-hover);
    color: var(--text-primary);
    padding: 10px 14px;
  }
  .btn-primary {
    background: var(--text-emphasis);
    color: var(--bg-void);
    padding: 10px 16px;
    font-weight: 600;
  }
  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
