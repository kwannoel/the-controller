<script lang="ts">
  import { onMount } from "svelte";

  interface Props {
    currentName: string;
    onSubmit: (newName: string) => void;
    onClose: () => void;
  }

  let { currentName, onSubmit, onClose }: Props = $props();

  let baseName = $derived(currentName.replace(/\.md$/, ""));
  let name = $state(currentName.replace(/\.md$/, ""));
  let nameInput: HTMLInputElement | undefined = $state();

  onMount(() => {
    if (nameInput) {
      nameInput.focus();
      nameInput.select();
    }
  });

  function submit() {
    if (!name.trim() || name.trim() === baseName) return;
    onSubmit(name.trim());
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

<div class="overlay" onclick={onClose} onkeydown={handleKeydown} role="dialog">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal" onclick={(e) => e.stopPropagation()} role="presentation">
    <div class="modal-header">Rename Note</div>
    <input
      bind:this={nameInput}
      bind:value={name}
      placeholder="Note name"
      class="input"
    />
    <div class="actions">
      <button class="btn-cancel" onclick={onClose}>Cancel</button>
      <button
        class="btn-primary"
        onclick={submit}
        disabled={!name.trim() || name.trim() === baseName}
      >
        Rename
      </button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 20vh;
    z-index: 100;
  }
  .modal {
    background: #1e1e2e;
    border: 1px solid #313244;
    border-radius: 8px;
    width: 380px;
    padding: 20px 24px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .modal-header {
    font-size: 14px;
    font-weight: 600;
    color: #cdd6f4;
  }
  .input {
    background: #11111b;
    color: #cdd6f4;
    border: 1px solid #313244;
    padding: 10px 12px;
    border-radius: 6px;
    font-size: 14px;
    outline: none;
    width: 100%;
    box-sizing: border-box;
  }
  .input:focus {
    border-color: #89b4fa;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .btn-cancel {
    background: #313244;
    color: #cdd6f4;
    border: none;
    padding: 10px 16px;
    border-radius: 6px;
    font-size: 13px;
    cursor: pointer;
  }
  .btn-cancel:hover {
    background: #45475a;
  }
  .btn-primary {
    background: #89b4fa;
    color: #1e1e2e;
    border: none;
    padding: 10px 16px;
    border-radius: 6px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
  }
  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
