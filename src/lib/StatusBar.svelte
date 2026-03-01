<script lang="ts">
  import { leaderActive } from "./stores";

  let visible: boolean = $state(false);

  leaderActive.subscribe((value) => {
    visible = value;
  });

  const hints = [
    { key: "ESC", label: "", separator: true },
    { key: "1-9", label: "session" },
    { key: "j/k", label: "next/prev" },
    { key: "c", label: "new session" },
    { key: "x", label: "close" },
    { key: "f", label: "find" },
    { key: "n", label: "new project" },
    { key: "?", label: "help" },
  ];
</script>

<div class="status-bar" class:visible>
  <div class="hints">
    {#each hints as hint}
      {#if hint.separator}
        <span class="hint">
          <kbd>{hint.key}</kbd>
          <span class="arrow">→</span>
        </span>
      {:else}
        <span class="hint">
          <kbd>{hint.key}</kbd>
          <span class="label">{hint.label}</span>
        </span>
      {/if}
    {/each}
  </div>
</div>

<style>
  .status-bar {
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    background: #1e1e2e;
    border-top: 1px solid #313244;
    z-index: 90;
    transform: translateY(100%);
    transition: transform 150ms ease-out;
    pointer-events: none;
  }

  .status-bar.visible {
    transform: translateY(0);
    pointer-events: auto;
  }

  .hints {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 6px 16px;
    flex-wrap: nowrap;
    white-space: nowrap;
  }

  .hint {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
  }

  kbd {
    color: #89b4fa;
    font-family: inherit;
    font-size: 12px;
    font-weight: 500;
  }

  .label {
    color: #6c7086;
  }

  .arrow {
    color: #6c7086;
  }
</style>
