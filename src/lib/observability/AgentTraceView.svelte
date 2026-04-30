<script lang="ts">
  import type { AgentTurnTrace, Chat, ChatWorkspaceLink, DaemonSession, EventRecord } from "../daemon/types";
  import {
    eventSummary,
    formatDuration,
    formatMetricValue,
    tokenTotal,
  } from "./turn-format";

  let {
    session,
    traces = [],
    chats = new Map<string, Chat>(),
    workspaceLinks = [],
    loading = false,
    error = null,
  }: {
    session: DaemonSession | null;
    traces?: AgentTurnTrace[];
    chats?: Map<string, Chat> | { get(key: string): Chat | undefined };
    workspaceLinks?: ChatWorkspaceLink[];
    loading?: boolean;
    error?: string | null;
  } = $props();

  const sortedTraces = $derived([...traces].sort((a, b) => (
    b.turn.received_at - a.turn.received_at || b.turn.id.localeCompare(a.turn.id)
  )));
  const latestTrace = $derived(sortedTraces[0] ?? null);

  function chatTitle(chatId: string): string {
    return chats.get(chatId)?.title ?? chatId;
  }

  function eventPayload(event: EventRecord): string {
    if (typeof event.payload === "string") return event.payload;
    try {
      return JSON.stringify(event.payload, null, 2);
    } catch {
      return String(event.payload);
    }
  }
</script>

<section class="trace-view" aria-label="Agent Trace">
  {#if session}
    <header class="trace-header">
      <div>
        <p class="eyebrow">Agent Trace</p>
        <h1>{session.label}</h1>
      </div>
      <div class="header-meta">
        <span>{session.agent}</span>
        <span>{session.status}</span>
      </div>
    </header>

    {#if error}
      <p class="trace-error" role="alert">{error}</p>
    {/if}

    <div class="context-strip">
      <div>
        <span>Current turn</span>
        <strong>{latestTrace?.turn.id ?? "none"}</strong>
      </div>
      <div>
        <span>Turns</span>
        <strong>{formatMetricValue(traces.length, "turns")}</strong>
      </div>
      <div>
        <span>Runtime</span>
        <strong>{session.native_session_id ?? "unavailable"}</strong>
      </div>
      {#if loading}
        <div>
          <span>State</span>
          <strong>loading</strong>
        </div>
      {/if}
    </div>

    <section class="links" aria-label="Linked context">
      <div>
        <h2>Linked Chats</h2>
        <div class="link-list">
          {#each [...new Set(traces.map((trace) => trace.turn.chat_id))] as chatId}
            <span>{chatTitle(chatId)}</span>
          {:else}
            <span class="muted">No linked chats</span>
          {/each}
        </div>
      </div>
      <div>
        <h2>Linked Workspaces</h2>
        <div class="link-list">
          {#each workspaceLinks as link (link.id)}
            <span>{link.label}</span>
          {:else}
            <span class="muted">No linked workspaces</span>
          {/each}
        </div>
      </div>
    </section>

    <section class="turn-list" aria-label="Turn list">
      {#each sortedTraces as trace, index (trace.turn.id)}
        <details class="turn-row" data-testid="turn-row" open={index === 0}>
          <summary>
            <span class="turn-id">{trace.turn.id}</span>
            <span>{trace.turn.status}</span>
            <span>{chatTitle(trace.turn.chat_id)}</span>
            <span>{trace.turn.ended_at == null ? "active" : formatDuration(trace.turn.received_at, trace.turn.ended_at)}</span>
            <span>{formatMetricValue(tokenTotal(trace.metrics), "tokens")}</span>
            <span>{formatMetricValue(trace.metrics?.tool_call_count, "tools")}</span>
          </summary>
          <div class="turn-detail">
            <div class="metric-grid">
              <span>Input {formatMetricValue(trace.metrics?.input_tokens, "tokens")}</span>
              <span>Output {formatMetricValue(trace.metrics?.output_tokens, "tokens")}</span>
              <span>Outbox {formatMetricValue(trace.metrics?.outbox_write_count, "outbox")}</span>
              <span>Errors {formatMetricValue(trace.metrics?.error_count, "errors")}</span>
            </div>
            <div class="event-list">
              {#each trace.events as event (`${event.session_id}:${event.seq}`)}
                <article class="event">
                  <div class="event-heading">
                    <span>{event.channel}</span>
                    <strong>{event.kind.replaceAll("_", " ")}</strong>
                  </div>
                  <p>{eventSummary(event)}</p>
                  <pre>{eventPayload(event)}</pre>
                </article>
              {:else}
                <p class="muted">No runtime events recorded for this turn.</p>
              {/each}
            </div>
          </div>
        </details>
      {:else}
        <p class="empty">No turns recorded for this agent.</p>
      {/each}
    </section>
  {:else}
    <div class="empty">Select an agent to inspect.</div>
  {/if}
</section>

<style>
  .trace-view {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 14px;
    overflow: auto;
    padding: 16px;
    background: var(--bg-void);
    color: var(--text-primary);
  }

  .trace-header,
  .context-strip,
  .links {
    border: 1px solid var(--border-default);
    border-radius: 8px;
    background: var(--bg-base);
  }

  .trace-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 14px;
  }

  .eyebrow,
  .header-meta,
  .context-strip span,
  h2,
  .event-heading span {
    color: var(--text-secondary);
    font: 11px var(--font-mono);
    text-transform: uppercase;
  }

  h1,
  h2,
  p {
    margin: 0;
  }

  h1 {
    color: var(--text-primary);
    font: 600 20px var(--font-mono);
  }

  .header-meta {
    display: flex;
    gap: 8px;
  }

  .header-meta span {
    border: 1px solid var(--border-default);
    border-radius: 4px;
    padding: 4px 6px;
    color: var(--text-emphasis);
  }

  .trace-error {
    border: 1px solid var(--status-error);
    border-radius: 6px;
    padding: 8px 10px;
    color: var(--status-error);
    font-size: 12px;
  }

  .context-strip {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    overflow: hidden;
  }

  .context-strip div {
    display: grid;
    gap: 6px;
    padding: 12px;
    border-right: 1px solid var(--border-default);
  }

  .context-strip div:last-child {
    border-right: 0;
  }

  .context-strip strong {
    overflow: hidden;
    color: var(--text-primary);
    font: 600 14px var(--font-mono);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .links {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0;
    overflow: hidden;
  }

  .links > div {
    display: grid;
    gap: 8px;
    min-width: 0;
    padding: 12px;
  }

  .links > div:first-child {
    border-right: 1px solid var(--border-default);
  }

  .link-list {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .link-list span {
    max-width: 220px;
    overflow: hidden;
    border: 1px solid var(--border-default);
    border-radius: 4px;
    padding: 4px 6px;
    background: var(--bg-surface);
    color: var(--text-primary);
    font: 12px var(--font-mono);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .turn-list {
    display: grid;
    gap: 8px;
  }

  .turn-row {
    border: 1px solid var(--border-default);
    border-radius: 8px;
    background: var(--bg-base);
    overflow: hidden;
  }

  summary {
    display: grid;
    grid-template-columns: minmax(110px, 1.2fr) repeat(5, minmax(72px, 0.8fr));
    gap: 10px;
    align-items: center;
    min-width: 0;
    padding: 10px 12px;
    color: var(--text-primary);
    font: 12px var(--font-mono);
    cursor: pointer;
  }

  summary:hover {
    background: var(--bg-hover);
  }

  .turn-id {
    overflow: hidden;
    color: var(--text-emphasis);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .turn-detail {
    display: grid;
    gap: 10px;
    border-top: 1px solid var(--border-default);
    padding: 12px;
  }

  .metric-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;
    color: var(--text-secondary);
    font: 12px var(--font-mono);
  }

  .event-list {
    display: grid;
    gap: 8px;
  }

  .event {
    display: grid;
    gap: 6px;
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    padding: 10px;
    background: var(--bg-surface);
  }

  .event-heading {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .event-heading strong {
    color: var(--text-primary);
    font: 12px var(--font-mono);
  }

  .event p {
    color: var(--text-primary);
    font-size: 13px;
  }

  pre {
    overflow: auto;
    max-height: 180px;
    margin: 0;
    color: var(--text-secondary);
    font: 11px/1.45 var(--font-mono);
    white-space: pre-wrap;
  }

  .muted,
  .empty {
    color: var(--text-secondary);
    font-size: 13px;
  }

  .empty {
    padding: 16px;
  }

  @media (max-width: 900px) {
    .context-strip,
    .links,
    .metric-grid {
      grid-template-columns: 1fr;
    }

    .context-strip div,
    .links > div:first-child {
      border-right: 0;
      border-bottom: 1px solid var(--border-default);
    }

    summary {
      grid-template-columns: 1fr 1fr;
    }
  }
</style>
