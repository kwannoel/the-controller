<script lang="ts">
  import { onMount } from "svelte";
  import { fromStore } from "svelte/store";
  import { getHelpSections } from "./commands";
  import { workspaceMode } from "./stores";

  interface Props {
    onClose: () => void;
  }

  let { onClose }: Props = $props();

  const workspaceModeState = fromStore(workspaceMode);
  const sections = $derived(getHelpSections(workspaceModeState.current));

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    }
  }

  onMount(() => {
    window.addEventListener("keydown", handleKeydown, { capture: true });
    return () => {
      window.removeEventListener("keydown", handleKeydown, { capture: true });
    };
  });
</script>

<div class="overlay" onclick={onClose} onkeydown={handleKeydown} role="dialog">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal" onclick={(e) => e.stopPropagation()} role="presentation">
    <div class="modal-header">Keyboard Shortcuts</div>
    <p class="subtitle">Mode: {workspaceModeState.current === "agents" ? "Agents" : "Development"} — Press Space to switch</p>
    <div class="sections-grid">
      {#each sections as section}
        <div class="section">
          <div class="section-label">{section.label}</div>
          <table class="shortcut-table">
            <tbody>
              {#each section.entries as { key, description }}
                <tr>
                  <td class="key-cell"><kbd>{key}</kbd></td>
                  <td class="desc-cell">{description}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/each}
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
    padding-top: 15vh;
    z-index: 100;
  }
  .modal {
    background: #1e1e2e;
    border: 1px solid #313244;
    border-radius: 8px;
    width: 720px;
    max-height: 70vh;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    overflow-y: auto;
  }
  .modal-header {
    font-size: 16px;
    font-weight: 600;
    color: #cdd6f4;
  }
  .subtitle {
    color: #6c7086;
    font-size: 13px;
    margin: 0;
  }
  .sections-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 20px;
  }
  .section-label {
    color: #a6adc8;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    padding: 0 8px 6px;
    border-bottom: 1px solid #313244;
    margin-bottom: 2px;
  }
  .shortcut-table {
    width: 100%;
    border-collapse: collapse;
  }
  .shortcut-table td {
    padding: 5px 8px;
  }
  .shortcut-table tr:not(:last-child) td {
    border-bottom: 1px solid rgba(49, 50, 68, 0.5);
  }
  .key-cell {
    width: 80px;
  }
  kbd {
    background: #ffffff;
    color: #1e1e2e;
    padding: 2px 8px;
    border-radius: 4px;
    font-family: monospace;
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
  }
  .desc-cell {
    color: #cdd6f4;
    font-size: 13px;
  }
</style>
