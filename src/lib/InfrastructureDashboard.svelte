<script lang="ts">
</script>

<div class="container">
  <div class="empty-state">
    <div class="title">Infrastructure</div>
    <div class="subtitle">No services deployed yet</div>
    <div class="hint">Deploy a project with <kbd>Leader</kbd> → <kbd>d</kbd> from a session</div>
  </div>
</div>

<style>
  .container {
    height: 100%;
    background: #11111b;
    color: #cdd6f4;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .empty-state {
    text-align: center;
  }

  .title {
    font-size: 18px;
    font-weight: 600;
    margin-bottom: 8px;
  }

  .subtitle {
    font-size: 14px;
    color: #a6adc8;
    margin-bottom: 16px;
  }

  .hint {
    font-size: 12px;
    color: #6c7086;
  }

  kbd {
    background: #313244;
    padding: 2px 6px;
    border-radius: 3px;
    font-family: monospace;
    font-size: 11px;
  }
</style>
