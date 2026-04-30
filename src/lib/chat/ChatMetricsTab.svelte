<script lang="ts">
  import type {
    AgentProfile,
    AgentTurnTrace,
    ChatAgentLink,
    ChatAgentMetrics,
    ChatMetrics,
    ChatTurnMetrics,
    DaemonSession,
  } from "../daemon/types";
  import {
    formatDurationMs,
    formatMetricValue,
    tokenTotal,
    traceMetricTotal,
    traceTokenTotal,
  } from "../observability/turn-format";

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

  const totalTokens = $derived(chatTokenTotal());

  function handleFromTokenSource(source: string | null): string | null {
    return source?.match(/^[@%]([A-Za-z0-9_-]+)$/)?.[1] ?? null;
  }

  function chatTokenTotal(): number | null {
    if (!metrics) return null;
    const turnRows = metrics.turns ?? [];
    if (turnRows.length > 0 && turnRows.every((turn) => turn.total_tokens == null)) {
      return null;
    }
    return metrics.input_tokens + metrics.output_tokens + metrics.cache_read_tokens + metrics.cache_write_tokens;
  }

  function routeTokenFromParts(routeType: string, tokenSource: string | null, profileId: string, profile: ProfileLite | undefined): string {
    const sigil = routeType === "shadow" ? "%" : "@";
    return `${sigil}${profile?.handle ?? handleFromTokenSource(tokenSource) ?? profileId}`;
  }

  function routeToken(link: ChatAgentLink, profile: ProfileLite | undefined): string {
    return routeTokenFromParts(link.route_type, link.token_source, link.profile_id, profile);
  }

  function agentName(link: ChatAgentLink): string {
    const profile = profiles.get(link.profile_id);
    return profile?.name ?? handleFromTokenSource(link.token_source) ?? link.profile_id;
  }

  function linkForSession(sessionId: string): ChatAgentLink | null {
    return agentLinks.find((link) => link.session_id === sessionId) ?? null;
  }

  function nameForAgent(profileId: string, sessionId: string, tokenSource: string | null): string {
    const profile = profiles.get(profileId);
    if (profile) return profile.name;
    const tokenHandle = handleFromTokenSource(tokenSource);
    if (tokenHandle) return tokenHandle;
    return sessions.get(sessionId)?.label ?? sessionId;
  }

  function agentRowsFromMetrics(rows: ChatAgentMetrics[]) {
    return rows.map((row) => {
      const profile = profiles.get(row.profile_id);
      return {
        sessionId: row.session_id,
        name: nameForAgent(row.profile_id, row.session_id, row.token_source),
        token: routeTokenFromParts(row.route_type, row.token_source, row.profile_id, profile),
        sessionStatus: row.status ?? sessions.get(row.session_id)?.status ?? "unknown",
        turns: row.turn_count,
        tokens: row.total_tokens,
        tools: row.tool_call_count,
        outbox: row.outbox_write_count,
        errors: row.error_count,
        duration: row.total_elapsed_ms,
        currentTurnId: row.current_turn_id,
      };
    });
  }

  function agentRowsFromLinks() {
    return agentLinks.map((link) => {
      const trace = traces.get(link.session_id) ?? [];
      const profile = profiles.get(link.profile_id);
      return {
        sessionId: link.session_id,
        name: agentName(link),
        token: routeToken(link, profile),
        sessionStatus: sessions.get(link.session_id)?.status ?? "unknown",
        turns: trace.length,
        tokens: traceTokenTotal(trace),
        tools: traceMetricTotal(trace, "tool_call_count"),
        outbox: traceMetricTotal(trace, "outbox_write_count"),
        errors: traceMetricTotal(trace, "error_count"),
        duration: traceDurationTotal(trace),
        currentTurnId: trace.at(-1)?.turn.id ?? null,
      };
    });
  }

  function agentRows() {
    return metrics?.agents?.length ? agentRowsFromMetrics(metrics.agents) : agentRowsFromLinks();
  }

  function traceDurationTotal(trace: AgentTurnTrace[]): number | null {
    let total = 0;
    let sawDuration = false;
    for (const item of trace) {
      if (item.turn.ended_at == null) continue;
      sawDuration = true;
      total += Math.max(0, item.turn.ended_at - item.turn.received_at);
    }
    return sawDuration ? total : null;
  }

  function turnRows(): ChatTurnMetrics[] {
    if (metrics?.turns) return metrics.turns;
    return traceLists().flatMap((trace) => trace.map((item) => ({
      turn_id: item.turn.id,
      session_id: item.turn.session_id,
      chat_id: item.turn.chat_id,
      chat_message_id: item.turn.chat_message_id,
      status: item.turn.status,
      received_at: item.turn.received_at,
      activity_started_at: item.turn.activity_started_at,
      ended_at: item.turn.ended_at,
      activity_latency_ms: item.turn.activity_started_at == null
        ? null
        : Math.max(0, item.turn.activity_started_at - item.turn.received_at),
      duration_ms: item.turn.ended_at == null ? null : Math.max(0, item.turn.ended_at - item.turn.received_at),
      input_tokens: item.metrics?.input_tokens ?? null,
      output_tokens: item.metrics?.output_tokens ?? null,
      cache_read_tokens: item.metrics?.cache_read_tokens ?? null,
      cache_write_tokens: item.metrics?.cache_write_tokens ?? null,
      total_tokens: tokenTotal(item.metrics),
      tool_call_count: item.metrics?.tool_call_count ?? null,
      outbox_write_count: item.metrics?.outbox_write_count ?? null,
      error_count: item.metrics?.error_count ?? null,
      updated_at: item.metrics?.updated_at ?? item.turn.ended_at ?? item.turn.activity_started_at ?? item.turn.received_at,
    }))).sort((a, b) => b.received_at - a.received_at || b.turn_id.localeCompare(a.turn_id));
  }

  function traceLists(): AgentTurnTrace[][] {
    const chatId = metrics?.chat_id ?? null;
    return agentLinks
      .map((link) => traces.get(link.session_id) ?? [])
      .map((trace) => chatId ? trace.filter((item) => item.turn.chat_id === chatId) : trace);
  }

  function turnAgentName(row: ChatTurnMetrics): string {
    const link = linkForSession(row.session_id);
    return nameForAgent(link?.profile_id ?? row.session_id, row.session_id, link?.token_source ?? null);
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
    <div class="total-cell">
      <span>Elapsed</span>
      <strong>{formatDurationMs(metrics?.total_elapsed_ms)}</strong>
    </div>
    <div class="total-cell">
      <span>Slowest</span>
      <strong>{formatDurationMs(metrics?.slowest_turn_ms)}</strong>
    </div>
  </div>

  <div class="agent-table" aria-label="Agent metrics">
    <div class="table-head agent-head">
      <span>Agent</span>
      <span>Status</span>
      <span>Turns</span>
      <span>Tokens</span>
      <span>Tools</span>
      <span>Outbox</span>
      <span>Errors</span>
    </div>
    {#each agentRows() as row (row.sessionId)}
      <button
        type="button"
        class="agent-row"
        aria-label={`Open ${row.name} trace`}
        onclick={() => onOpenAgent(row.sessionId)}
      >
        <span class="agent-cell">
          <strong>{row.name}</strong>
          <small>{row.token}</small>
        </span>
        <span class="status">{row.sessionStatus}</span>
        <span>{formatMetricValue(row.turns, "turns")}</span>
        <span>{formatMetricValue(row.tokens, "tokens")}</span>
        <span>{formatMetricValue(row.tools, "tools")}</span>
        <span>{formatMetricValue(row.outbox, "outbox")}</span>
        <span>{formatMetricValue(row.errors, "errors")}</span>
      </button>
    {:else}
      <p class="empty">No linked agents yet.</p>
    {/each}
  </div>

  <div class="section-heading">
    <h3>Turn Metrics</h3>
    <span>{formatDurationMs(metrics?.average_turn_ms)} average</span>
  </div>
  <div class="turn-table" aria-label="Turn metrics">
    <div class="table-head turn-head">
      <span>Turn</span>
      <span>Agent</span>
      <span>Status</span>
      <span>Duration</span>
      <span>Tokens</span>
      <span>Tools</span>
      <span>Outbox</span>
      <span>Errors</span>
    </div>
    {#each turnRows() as row (row.turn_id)}
      <div class="turn-row">
        <span class="turn-id">{row.turn_id}</span>
        <span>{turnAgentName(row)}</span>
        <span class="status">{row.status}</span>
        <span>{formatDurationMs(row.duration_ms)}</span>
        <span>{formatMetricValue(row.total_tokens, "tokens")}</span>
        <span>{formatMetricValue(row.tool_call_count, "tools")}</span>
        <span>{formatMetricValue(row.outbox_write_count, "outbox")}</span>
        <span>{formatMetricValue(row.error_count, "errors")}</span>
      </div>
    {:else}
      <p class="empty">No turns recorded for this chat.</p>
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

  h2,
  h3 {
    margin: 0;
    font: 600 16px var(--font-mono);
  }

  h3 {
    font-size: 13px;
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
    grid-template-columns: repeat(6, minmax(0, 1fr));
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

  .agent-table,
  .turn-table {
    display: grid;
    border: 1px solid var(--border-default);
    border-radius: 8px;
    overflow: hidden;
    background: var(--bg-base);
  }

  .table-head,
  .agent-row,
  .turn-row {
    display: grid;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }

  .agent-head,
  .agent-row {
    grid-template-columns: minmax(160px, 1.4fr) repeat(6, minmax(66px, 0.7fr));
  }

  .turn-head,
  .turn-row {
    grid-template-columns: minmax(120px, 1.1fr) minmax(120px, 1fr) repeat(6, minmax(66px, 0.7fr));
  }

  .table-head {
    padding: 9px 12px;
    border-bottom: 1px solid var(--border-default);
  }

  .agent-row,
  .turn-row {
    width: 100%;
    border-bottom: 1px solid var(--border-subtle);
    padding: 10px 12px;
    background: transparent;
    color: var(--text-primary);
    font: 12px var(--font-mono);
    text-align: left;
  }

  .agent-row {
    border: 0;
    border-bottom: 1px solid var(--border-subtle);
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

  .turn-row:last-of-type {
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

  .section-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    color: var(--text-primary);
  }

  .section-heading span {
    color: var(--text-secondary);
    font: 11px var(--font-mono);
    text-transform: uppercase;
  }

  .turn-id {
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

    .table-head {
      display: none;
    }

    .agent-row,
    .turn-row {
      grid-template-columns: 1fr 1fr;
    }
  }
</style>
