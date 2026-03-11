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
    switch (status) {
      case "running": return "#a6e3a1";
      case "stopped": return "#6c7086";
      case "deploying": return "#89b4fa";
      case "error": return "#f38ba8";
      default: return "#a6adc8";
    }
  }
</script>

<div class="container">
  {#if services.length === 0}
    <div class="empty-state">
      <div class="title">Infrastructure</div>
      <div class="subtitle">No services deployed yet</div>
      <div class="hint">Deploy a project with <kbd>d</kbd> from the infrastructure workspace</div>
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
    background: #11111b;
    color: #cdd6f4;
    display: flex;
  }

  .empty-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
  }

  .title { font-size: 18px; font-weight: 600; margin-bottom: 8px; }
  .subtitle { font-size: 14px; color: #a6adc8; margin-bottom: 16px; }
  .hint { font-size: 12px; color: #6c7086; }
  kbd { background: #313244; padding: 2px 6px; border-radius: 3px; font-family: monospace; font-size: 11px; }

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
    background: #1e1e2e;
    border: 1px solid #313244;
    border-radius: 6px;
    padding: 12px 16px;
    cursor: pointer;
    text-align: left;
    color: #cdd6f4;
    font-family: inherit;
    font-size: 13px;
  }

  .service-card:hover { border-color: #45475a; }
  .service-card.selected { border-color: #89b4fa; background: rgba(137, 180, 250, 0.05); }

  .service-header { display: flex; align-items: center; gap: 8px; }
  .status-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
  .service-name { font-weight: 600; flex: 1; }
  .service-status { font-size: 12px; color: #a6adc8; }

  .service-meta { display: flex; gap: 12px; margin-top: 6px; font-size: 11px; color: #6c7086; }

  .log-panel {
    flex: 1;
    background: #1e1e2e;
    border: 1px solid #313244;
    border-radius: 6px;
    display: flex;
    flex-direction: column;
    min-height: 200px;
  }

  .log-header {
    padding: 8px 12px;
    border-bottom: 1px solid #313244;
    font-size: 12px;
    color: #a6adc8;
  }

  .log-content {
    flex: 1;
    padding: 8px 12px;
    overflow-y: auto;
    font-family: monospace;
    font-size: 12px;
    color: #a6adc8;
  }

  .log-line { padding: 1px 0; }
</style>
