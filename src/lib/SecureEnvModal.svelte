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
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 18vh;
    z-index: 120;
    outline: none;
  }
  .modal {
    background: #1e1e2e;
    border: 1px solid #313244;
    border-radius: 8px;
    width: 420px;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    box-shadow: 0 24px 48px rgba(0, 0, 0, 0.35);
  }
  .modal-header {
    font-size: 16px;
    font-weight: 600;
    color: #cdd6f4;
  }
  .meta {
    display: grid;
    gap: 8px;
    background: #181825;
    border: 1px solid #313244;
    border-radius: 6px;
    padding: 12px;
  }
  .meta-row {
    display: flex;
    justify-content: space-between;
    gap: 16px;
  }
  .meta-label {
    color: #7f849c;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .meta-value {
    color: #cdd6f4;
    font-size: 13px;
  }
  .meta-value.code {
    font-family: "SFMono-Regular", "Menlo", monospace;
  }
  .label {
    color: #bac2de;
    font-size: 13px;
  }
  .input-row {
    display: flex;
    gap: 8px;
  }
  .input {
    flex: 1;
    background: #11111b;
    color: #cdd6f4;
    border: 1px solid #45475a;
    padding: 10px 12px;
    border-radius: 6px;
    font-size: 14px;
    outline: none;
  }
  .input:focus {
    border-color: #89b4fa;
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
    background: #313244;
    color: #cdd6f4;
    padding: 10px 14px;
  }
  .btn-primary {
    background: #89b4fa;
    color: #1e1e2e;
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
