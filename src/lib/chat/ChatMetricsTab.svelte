<script lang="ts">
  import type { AgentProfile, AgentTurnTrace, ChatAgentLink, ChatMetrics, DaemonSession } from "../daemon/types";
  import { formatMetricValue, traceMetricTotal, traceTokenTotal } from "../observability/turn-format";

  type ProfileLite = Pick<AgentProfile, "id" | "handle" | "name">;

  let {
    metrics = null,
    agentLinks = [],
    sessions = new Map<string, DaemonSession>(),
    profiles = new Map<string, ProfileLite>(),
    traces = new Map<string, AgentTurnTrace[]>(),
    loading = false,
    error = null,
    onOpenAgent = () => {},
  }: {
    metrics?: ChatMetrics | null;
    agentLinks?: ChatAgentLink[];
    sessions?: Map<string, DaemonSession> | { get(key: string): DaemonSession | undefined };
    profiles?: Map<string, ProfileLite> | { get(key: string): ProfileLite | undefined };
    traces?: Map<string, AgentTurnTrace[]> | { get(key: string): AgentTurnTrace[] | undefined };
    loading?: boolean;
    error?: string | null;
    onOpenAgent?: (sessionId: string) => void;
  } = $props();

  const totalTokens = $derived(metrics
    ? metrics.input_tokens + metrics.output_tokens + metrics.cache_read_tokens + metrics.cache_write_tokens
    : null);

  function handleFromTokenSource(source: string | null): string | null {
    return source?.match(/^[@%]([A-Za-z0-9_-]+)$/)?.[1] ?? null;
  }

  function routeToken(link: ChatAgentLink, profile: ProfileLite | undefined): string {
    const sigil = link.route_type === "shadow" ? "%" : "@";
    return `${sigil}${profile?.handle ?? handleFromTokenSource(link.token_source) ?? link.profile_id}`;
  }

  function agentName(link: ChatAgentLink): string {
    const profile = profiles.get(link.profile_id);
    return profile?.name ?? handleFromTokenSource(link.token_source) ?? link.profile_id;
  }

  function agentRows() {
    return agentLinks.map((link) => {
      const trace = traces.get(link.session_id) ?? [];
      const profile = profiles.get(link.profile_id);
      return {
        link,
        name: agentName(link),
        token: routeToken(link, profile),
        sessionStatus: sessions.get(link.session_id)?.status ?? "unknown",
        turns: trace.length,
        tokens: traceTokenTotal(trace),
        tools: traceMetricTotal(trace, "tool_call_count"),
        errors: traceMetricTotal(trace, "error_count"),
      };
    });
  }
</script>

<section class="metrics-tab" aria-label="Chat Metrics">
  <div class="metrics-header">
    <div>
      <h2>Metrics</h2>
      <p>Cost and runtime facts for this chat.</p>
    </div>
    {#if loading}
      <span class="load-state">loading</span>
    {/if}
  </div>

  {#if error}
    <p class="metrics-error" role="alert">{error}</p>
  {/if}

  <div class="totals" aria-label="Chat metric totals">
    <div class="total-cell">
      <span>Tokens</span>
      <strong>{formatMetricValue(totalTokens, "tokens")}</strong>
    </div>
    <div class="total-cell">
      <span>Turns</span>
      <strong>{formatMetricValue(metrics?.turn_count, "turns")}</strong>
    </div>
    <div class="total-cell">
      <span>Tools</span>
      <strong>{formatMetricValue(metrics?.tool_call_count, "tools")}</strong>
    </div>
    <div class="total-cell">
      <span>Errors</span>
      <strong>{formatMetricValue(metrics?.error_count, "errors")}</strong>
    </div>
  </div>

  <div class="agent-table" aria-label="Agent metrics">
    <div class="table-head">
      <span>Agent</span>
      <span>Status</span>
      <span>Turns</span>
      <span>Tokens</span>
      <span>Tools</span>
      <span>Errors</span>
    </div>
    {#each agentRows() as row (row.link.id)}
      <button
        type="button"
        class="agent-row"
        aria-label={`Open ${row.name} trace`}
        onclick={() => onOpenAgent(row.link.session_id)}
      >
        <span class="agent-cell">
          <strong>{row.name}</strong>
          <small>{row.token}</small>
        </span>
        <span class="status">{row.sessionStatus}</span>
        <span>{formatMetricValue(row.turns, "turns")}</span>
        <span>{formatMetricValue(row.tokens, "tokens")}</span>
        <span>{formatMetricValue(row.tools, "tools")}</span>
        <span>{formatMetricValue(row.errors, "errors")}</span>
      </button>
    {:else}
      <p class="empty">No linked agents yet.</p>
    {/each}
  </div>
</section>

<style>
  .metrics-tab {
    min-height: 0;
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 14px;
    overflow: auto;
    padding: 14px;
    background: var(--bg-void);
    color: var(--text-primary);
  }

  .metrics-header {
    display: flex;
    align-items: start;
    justify-content: space-between;
    gap: 12px;
  }

  h2 {
    margin: 0;
    font: 600 16px var(--font-mono);
  }

  p {
    margin: 0;
  }

  .metrics-header p,
  .empty {
    color: var(--text-secondary);
    font-size: 12px;
  }

  .load-state {
    color: var(--text-emphasis);
    font: 11px var(--font-mono);
    text-transform: uppercase;
  }

  .metrics-error {
    border: 1px solid var(--status-error);
    border-radius: 6px;
    padding: 8px 10px;
    color: var(--status-error);
    font-size: 12px;
  }

  .totals {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    border: 1px solid var(--border-default);
    border-radius: 8px;
    overflow: hidden;
    background: var(--bg-base);
  }

  .total-cell {
    display: grid;
    gap: 6px;
    min-width: 0;
    padding: 12px;
    border-right: 1px solid var(--border-default);
  }

  .total-cell:last-child {
    border-right: 0;
  }

  .total-cell span,
  .table-head,
  .agent-row small {
    color: var(--text-secondary);
    font: 11px var(--font-mono);
    text-transform: uppercase;
  }

  .total-cell strong {
    color: var(--text-primary);
    font: 600 18px var(--font-mono);
  }

  .agent-table {
    display: grid;
    border: 1px solid var(--border-default);
    border-radius: 8px;
    overflow: hidden;
    background: var(--bg-base);
  }

  .table-head,
  .agent-row {
    display: grid;
    grid-template-columns: minmax(160px, 1.4fr) repeat(5, minmax(72px, 0.7fr));
    align-items: center;
    gap: 10px;
    min-width: 0;
  }

  .table-head {
    padding: 9px 12px;
    border-bottom: 1px solid var(--border-default);
  }

  .agent-row {
    width: 100%;
    border: 0;
    border-bottom: 1px solid var(--border-subtle);
    padding: 10px 12px;
    background: transparent;
    color: var(--text-primary);
    font: 12px var(--font-mono);
    text-align: left;
    cursor: pointer;
  }

  .agent-row:hover,
  .agent-row:focus-visible {
    background: var(--bg-hover);
    outline: none;
  }

  .agent-row:last-of-type {
    border-bottom: 0;
  }

  .agent-cell {
    display: grid;
    gap: 3px;
    min-width: 0;
  }

  .agent-cell strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .status {
    color: var(--text-emphasis);
  }

  .empty {
    padding: 12px;
  }

  @media (max-width: 860px) {
    .totals {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .total-cell:nth-child(2) {
      border-right: 0;
    }

    .total-cell:nth-child(-n + 2) {
      border-bottom: 1px solid var(--border-default);
    }

    .table-head {
      display: none;
    }

    .agent-row {
      grid-template-columns: 1fr 1fr;
    }
  }
</style>
