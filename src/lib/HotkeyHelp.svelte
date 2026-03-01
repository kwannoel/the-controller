<script lang="ts">
  import { onMount } from "svelte";

  interface Props {
    onClose: () => void;
  }

  let { onClose }: Props = $props();

  const shortcuts: { key: string; description: string }[] = [
    { key: "1-9", description: "Switch to session" },
    { key: "j / k", description: "Next / previous session" },
    { key: "c", description: "New session" },
    { key: "x", description: "Close session" },
    { key: "f", description: "Find project (fuzzy finder)" },
    { key: "n", description: "New project" },
    { key: "h", description: "Focus sidebar" },
    { key: "l", description: "Focus terminal" },
    { key: "J / K", description: "Next / previous project" },
    { key: "?", description: "Toggle this help" },
  ];

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
    <p class="subtitle">Press Escape to activate leader, then:</p>
    <table class="shortcut-table">
      <thead>
        <tr>
          <th class="col-key">Key</th>
          <th class="col-desc">Action</th>
        </tr>
      </thead>
      <tbody>
        {#each shortcuts as { key, description }}
          <tr>
            <td class="key-cell"><kbd>{key}</kbd></td>
            <td class="desc-cell">{description}</td>
          </tr>
        {/each}
      </tbody>
    </table>
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
    width: 420px;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 12px;
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
  .shortcut-table {
    width: 100%;
    border-collapse: collapse;
  }
  .shortcut-table th {
    text-align: left;
    color: #6c7086;
    font-size: 11px;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 6px 8px;
    border-bottom: 1px solid #313244;
  }
  .shortcut-table td {
    padding: 7px 8px;
  }
  .shortcut-table tr:not(:last-child) td {
    border-bottom: 1px solid rgba(49, 50, 68, 0.5);
  }
  .key-cell {
    width: 80px;
  }
  kbd {
    background: #313244;
    color: #89b4fa;
    padding: 2px 8px;
    border-radius: 4px;
    font-family: monospace;
    font-size: 13px;
    font-weight: 500;
    white-space: nowrap;
  }
  .desc-cell {
    color: #cdd6f4;
    font-size: 13px;
  }
</style>
