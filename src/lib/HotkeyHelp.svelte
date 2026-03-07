<script lang="ts">
  import { onMount } from "svelte";

  interface Props {
    onClose: () => void;
  }

  let { onClose }: Props = $props();

  const shortcuts: { key: string; description: string }[] = [
    { key: "j / k", description: "Next / previous item (project or session)" },
    { key: "J / K", description: "Next / previous project (skip sessions)" },
    { key: "l / Enter", description: "Expand/collapse project or focus terminal" },
    { key: "g", description: "Go to project / session (jump mode)" },
    { key: "c", description: "Create Claude session with issue" },
    { key: "x", description: "Create Codex session with issue" },
    { key: "C", description: "Background worker: Claude (autonomous)" },
    { key: "X", description: "Background worker: Codex (autonomous)" },
    { key: "d", description: "Delete focused item (session or project)" },
    { key: "a", description: "Archive focused item (session or project)" },
    { key: "A", description: "View archived projects" },
    { key: "m", description: "Merge session branch (create PR)" },
    { key: "f", description: "Find project (fuzzy finder)" },
    { key: "n", description: "New project" },
    { key: "s", description: "Toggle sidebar" },
    { key: "t", description: "Toggle GitHub issues panel" },
    { key: "i", description: "Create GitHub issue for focused project" },
    { key: "S", description: "Screenshot app → new session with image" },
    { key: "b", description: "Toggle background agent panel" },
    { key: "o", description: "Toggle maintainer on/off (when panel open)" },
    { key: "p", description: "Triage issues (swipe high/low priority)" },
    { key: "Esc", description: "Move focus up (terminal → session → project)" },
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
    <p class="subtitle">Keys work directly. Press Escape first when terminal is focused.</p>
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
