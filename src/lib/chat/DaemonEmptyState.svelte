<script lang="ts">
  let { onRetry }: { onRetry: () => void } = $props();
</script>

<div class="empty">
  <h2>Daemon not running</h2>
  <p>Start it with:</p>
  <pre><code>./target/release/the-controller-daemon</code></pre>
  <p class="muted">Expected token: <code>~/.the-controller/daemon.token</code></p>
  <button onclick={onRetry}>Retry</button>
</div>

<style>
  .empty { max-width: 560px; margin: 10vh auto; text-align: center; color: var(--text-secondary); }
  pre { background: var(--bg-elevated); padding: 12px; border-radius: 6px; }
  button { margin-top: 16px; padding: 6px 14px; }
  .muted { font-size: 12px; opacity: 0.7; }
</style>
