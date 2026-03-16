<script lang="ts">
  import { fromStore } from "svelte/store";
  import { deployedServices, selectedServiceId, serviceLogLines, type DeployedService } from "./deploy-stores";

  const servicesState = fromStore(deployedServices);
  const selectedState = fromStore(selectedServiceId);
  const logLinesState = fromStore(serviceLogLines);

  let services = $derived(servicesState.current);
  let selectedId = $derived(selectedState.current);
  let logs = $derived(logLinesState.current);
  let selectedService = $derived(services.find(s => s.uuid === selectedId) ?? null);

  function formatUptime(seconds: number): string {
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
    if (seconds < 86400) return `${Math.floor(seconds / 3600)}h`;
    return `${Math.floor(seconds / 86400)}d`;
  }

  function statusColor(status: string): string {
    const style = getComputedStyle(document.documentElement);
    switch (status) {
      case "running": return style.getPropertyValue("--status-idle").trim();
      case "stopped": return style.getPropertyValue("--text-tertiary").trim();
      case "deploying": return style.getPropertyValue("--status-working").trim();
      case "error": return style.getPropertyValue("--status-error").trim();
      default: return style.getPropertyValue("--text-secondary").trim();
    }
  }
</script>

<div class="container">
  {#if services.length === 0}
    <div class="empty-state">
      <div class="empty-title">No services deployed yet</div>
      <div class="empty-hint">press <kbd>d</kbd> to deploy a project</div>
    </div>
  {:else}
    <div class="dashboard">
      <div class="service-list">
        {#each services as service}
          <button
            class="service-card"
            class:selected={selectedId === service.uuid}
            onclick={() => selectedServiceId.set(service.uuid)}
          >
            <div class="service-header">
              <span class="status-dot" style="background: {statusColor(service.status)}"></span>
              <span class="service-name">{service.name}</span>
              <span class="service-status">{service.status}</span>
            </div>
            <div class="service-meta">
              {#if service.deployTarget === "cloudflare-pages"}
                <span class="meta-item">Cloudflare Pages</span>
              {:else}
                <span class="meta-item">CPU: {service.cpuPercent}%</span>
                <span class="meta-item">RAM: {service.memoryMb}MB</span>
                <span class="meta-item">{formatUptime(service.uptimeSeconds)} uptime</span>
              {/if}
            </div>
          </button>
        {/each}
      </div>

      <div class="log-panel">
        <div class="log-header">
          {#if selectedService}
            Logs — {selectedService.name}
          {:else}
            Select a service to view logs
          {/if}
        </div>
        <div class="log-content">
          {#each logs as line}
            <div class="log-line">{line}</div>
          {/each}
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .container {
    height: 100%;
    background: var(--bg-void);
    color: var(--text-primary);
    display: flex;
  }

  .empty-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
  }

  .empty-title { color: var(--text-primary); font-size: 16px; font-weight: 500; margin-bottom: 8px; }
  .empty-hint { color: var(--text-secondary); font-size: 13px; }
  .empty-hint kbd { background: var(--bg-hover); color: var(--text-emphasis); padding: 1px 6px; border-radius: 3px; font-family: var(--font-mono); font-size: 12px; }

  .dashboard {
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: 16px;
    gap: 12px;
  }

  .service-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .service-card {
    background: var(--bg-surface);
    border: 1px solid var(--border-default);
    border-radius: 6px;
    padding: 12px 16px;
    cursor: pointer;
    text-align: left;
    color: var(--text-primary);
    font-family: inherit;
    font-size: 13px;
  }

  .service-card:hover { border-color: var(--text-tertiary); }
  .service-card.selected { border-color: var(--focus-ring); background: rgba(255, 255, 255, 0.05); }

  .service-header { display: flex; align-items: center; gap: 8px; }
  .status-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
  .service-name { font-weight: 600; flex: 1; }
  .service-status { font-size: 12px; color: var(--text-secondary); }

  .service-meta { display: flex; gap: 12px; margin-top: 6px; font-size: 11px; color: var(--text-tertiary); }

  .log-panel {
    flex: 1;
    background: var(--bg-surface);
    border: 1px solid var(--border-default);
    border-radius: 6px;
    display: flex;
    flex-direction: column;
    min-height: 200px;
  }

  .log-header {
    padding: 8px 12px;
    border-bottom: 1px solid var(--border-default);
    font-size: 12px;
    color: var(--text-secondary);
  }

  .log-content {
    flex: 1;
    padding: 8px 12px;
    overflow-y: auto;
    font-family: monospace;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .log-line { padding: 1px 0; }
</style>
