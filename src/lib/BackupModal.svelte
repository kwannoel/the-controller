<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { save, open } from "@tauri-apps/plugin-dialog";
  import { showToast } from "./toast";

  interface Props {
    onClose: () => void;
  }

  let { onClose }: Props = $props();
  let modalEl: HTMLDivElement | undefined = $state();
  let passphrase = $state("");
  let confirmPassphrase = $state("");
  let mode: "export" | "import" = $state("export");
  let busy = $state(false);

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    }
  }

  onMount(() => {
    modalEl?.focus();
    window.addEventListener("keydown", handleKeydown, { capture: true });
    return () => {
      window.removeEventListener("keydown", handleKeydown, { capture: true });
    };
  });

  async function handleExport() {
    if (!passphrase) {
      showToast("Passphrase is required", "error");
      return;
    }
    if (passphrase !== confirmPassphrase) {
      showToast("Passphrases do not match", "error");
      return;
    }

    const path = await save({
      title: "Export Backup",
      defaultPath: "the-controller-backup.tcbk",
      filters: [{ name: "Backup", extensions: ["tcbk"] }],
    });

    if (!path) return;

    busy = true;
    try {
      await invoke("export_backup", { passphrase, path });
      showToast("Backup exported successfully", "info");
      onClose();
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      busy = false;
    }
  }

  async function handleImport() {
    if (!passphrase) {
      showToast("Passphrase is required", "error");
      return;
    }

    const path = await open({
      title: "Import Backup",
      filters: [{ name: "Backup", extensions: ["tcbk"] }],
      multiple: false,
      directory: false,
    });

    if (!path) return;

    busy = true;
    try {
      const count: number = await invoke("import_backup", { passphrase, path });
      showToast(`Imported ${count} project${count !== 1 ? "s" : ""} from backup`, "info");
      onClose();
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      busy = false;
    }
  }
</script>

<div class="overlay" onclick={onClose} role="dialog">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="modal"
    bind:this={modalEl}
    onclick={(e) => e.stopPropagation()}
    role="presentation"
    tabindex="-1"
  >
    <div class="modal-header">Backup</div>

    <div class="tabs">
      <button
        class="tab"
        class:active={mode === "export"}
        onclick={() => { mode = "export"; passphrase = ""; confirmPassphrase = ""; }}
      >Export</button>
      <button
        class="tab"
        class:active={mode === "import"}
        onclick={() => { mode = "import"; passphrase = ""; confirmPassphrase = ""; }}
      >Import</button>
    </div>

    <p class="description">
      {#if mode === "export"}
        Export all projects to an encrypted backup file.
      {:else}
        Import projects from an encrypted backup file.
      {/if}
    </p>

    <label class="field-label">
      Passphrase
      <input
        type="password"
        bind:value={passphrase}
        placeholder="Enter passphrase"
        class="input"
        onkeydown={(e) => { if (e.key === "Enter") { mode === "export" ? handleExport() : handleImport(); } }}
      />
    </label>

    {#if mode === "export"}
      <label class="field-label">
        Confirm passphrase
        <input
          type="password"
          bind:value={confirmPassphrase}
          placeholder="Confirm passphrase"
          class="input"
          onkeydown={(e) => { if (e.key === "Enter") handleExport(); }}
        />
      </label>
    {/if}

    <div class="actions">
      <button
        class="btn-confirm"
        onclick={() => { mode === "export" ? handleExport() : handleImport(); }}
        disabled={busy}
      >
        {busy ? "Working..." : mode === "export" ? "Export" : "Import"}
      </button>
      <button class="btn-cancel" onclick={onClose}>Cancel</button>
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
    padding-top: 20vh;
    z-index: 100;
  }
  .modal {
    background: #1e1e2e;
    border: 1px solid #313244;
    border-radius: 8px;
    width: 380px;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    outline: none;
  }
  .modal-header {
    font-size: 16px;
    font-weight: 600;
    color: #cdd6f4;
  }
  .tabs {
    display: flex;
    gap: 0;
    border: 1px solid #313244;
    border-radius: 6px;
    overflow: hidden;
  }
  .tab {
    flex: 1;
    background: none;
    border: none;
    color: #6c7086;
    padding: 8px 0;
    font-size: 13px;
    cursor: pointer;
    box-shadow: none;
  }
  .tab:hover {
    color: #cdd6f4;
    background: #313244;
  }
  .tab.active {
    color: #cdd6f4;
    background: #45475a;
  }
  .description {
    color: #a6adc8;
    font-size: 13px;
    margin: 0;
    line-height: 1.5;
  }
  .field-label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: #a6adc8;
  }
  .input {
    background: #313244;
    border: 1px solid #45475a;
    border-radius: 6px;
    color: #cdd6f4;
    padding: 8px 10px;
    font-size: 13px;
    outline: none;
  }
  .input:focus {
    border-color: #89b4fa;
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: 4px;
  }
  .btn-confirm {
    background: #89b4fa;
    color: #1e1e2e;
    border: none;
    padding: 10px 16px;
    border-radius: 6px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
  }
  .btn-confirm:hover {
    background: #b4d0fb;
  }
  .btn-confirm:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .btn-cancel {
    background: none;
    color: #6c7086;
    border: 1px solid #313244;
    padding: 10px 16px;
    border-radius: 6px;
    font-size: 13px;
    cursor: pointer;
    margin-left: auto;
  }
  .btn-cancel:hover {
    color: #cdd6f4;
    border-color: #45475a;
  }
</style>
