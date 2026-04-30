import type { AgentTurnTrace, EventRecord, TurnMetrics } from "../daemon/types";

type MetricKind = "tokens" | "tools" | "errors" | "turns" | "outbox";

const LABELS: Record<MetricKind, [string, string]> = {
  tokens: ["token", "tokens"],
  tools: ["tool", "tools"],
  errors: ["error", "errors"],
  turns: ["turn", "turns"],
  outbox: ["outbox write", "outbox writes"],
};

export function formatMetricValue(value: number | null | undefined, kind: MetricKind): string {
  if (value == null) return "unavailable";
  const [singular, plural] = LABELS[kind];
  return `${value} ${value === 1 ? singular : plural}`;
}

export function formatDuration(start: number | null | undefined, end: number | null | undefined): string {
  if (start == null || end == null) return "unavailable";
  const ms = Math.max(0, end - start);
  if (ms < 1_000) return `${ms}ms`;
  if (ms < 60_000) {
    const seconds = ms / 1_000;
    return `${Number.isInteger(seconds) ? seconds.toFixed(0) : seconds.toFixed(1)}s`;
  }
  const minutes = Math.floor(ms / 60_000);
  const seconds = Math.floor((ms % 60_000) / 1_000);
  return `${minutes}m ${seconds.toString().padStart(2, "0")}s`;
}

export function tokenTotal(metrics: TurnMetrics | null | undefined): number | null {
  if (!metrics) return null;
  const parts = [
    metrics.input_tokens,
    metrics.output_tokens,
    metrics.cache_read_tokens,
    metrics.cache_write_tokens,
  ];
  if (parts.every((part) => part == null)) return null;
  return parts.reduce<number>((sum, part) => sum + (part ?? 0), 0);
}

export function traceTokenTotal(trace: AgentTurnTrace[]): number | null {
  let sawMetrics = false;
  let total = 0;
  for (const item of trace) {
    const value = tokenTotal(item.metrics);
    if (value == null) continue;
    sawMetrics = true;
    total += value;
  }
  return sawMetrics ? total : null;
}

export function traceMetricTotal(
  trace: AgentTurnTrace[],
  field: "tool_call_count" | "outbox_write_count" | "error_count",
): number | null {
  let sawMetrics = false;
  let total = 0;
  for (const item of trace) {
    if (!item.metrics) continue;
    sawMetrics = true;
    total += item.metrics[field];
  }
  return sawMetrics ? total : null;
}

export function eventSummary(event: EventRecord): string {
  const payload = event.payload as Record<string, unknown> | null;
  if (payload && typeof payload === "object") {
    for (const key of ["text", "message", "delta", "reason", "code", "tool"]) {
      const value = payload[key];
      if (typeof value === "string" && value.trim()) return value;
    }
  }
  return event.kind.replaceAll("_", " ");
}
