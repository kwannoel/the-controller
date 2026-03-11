<script lang="ts">
  import { fromStore } from "svelte/store";
  import { toasts } from "./toast";

  const toastsState = fromStore(toasts);
  let toastList = $derived(toastsState.current);
</script>

{#if toastList.length > 0}
  <div class="toast-container">
    {#each toastList as toast (toast.id)}
      <div class="toast" class:error={toast.type === "error"} class:info={toast.type === "info"}>
        {toast.text}
      </div>
    {/each}
  </div>
{/if}

<style>
  .toast-container {
    position: fixed;
    bottom: 16px;
    right: 16px;
    z-index: 1000;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .toast {
    background: var(--bg-elevated);
    color: var(--text-primary);
    padding: 10px 16px;
    border-radius: 6px;
    font-size: 13px;
    max-width: 400px;
    border: 1px solid var(--border-default);
    border-left: 3px solid var(--text-emphasis);
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.5);
  }

  .toast.error {
    border-left-color: var(--status-error);
  }

  .toast.info {
    border-left-color: var(--text-emphasis);
  }
</style>
