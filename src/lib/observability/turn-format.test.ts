import { describe, expect, it } from "vitest";
import { formatDuration, formatDurationMs, formatMetricValue, tokenTotal } from "./turn-format";

describe("formatMetricValue", () => {
  it("distinguishes missing metrics from zero", () => {
    expect(formatMetricValue(null, "tokens")).toBe("unavailable");
    expect(formatMetricValue(0, "tokens")).toBe("0 tokens");
  });

  it("formats counts without pretending missing values are zero", () => {
    expect(formatMetricValue(undefined, "tools")).toBe("unavailable");
    expect(formatMetricValue(1, "tools")).toBe("1 tool");
    expect(formatMetricValue(3, "errors")).toBe("3 errors");
  });
});

describe("formatDuration", () => {
  it("distinguishes unobserved timing from zero duration", () => {
    expect(formatDuration(null, 10)).toBe("unavailable");
    expect(formatDuration(10, 10)).toBe("0ms");
    expect(formatDuration(10, 1_510)).toBe("1.5s");
  });

  it("formats duration fields that already arrive as elapsed milliseconds", () => {
    expect(formatDurationMs(null)).toBe("unavailable");
    expect(formatDurationMs(0)).toBe("0ms");
    expect(formatDurationMs(61_000)).toBe("1m 01s");
  });
});

describe("tokenTotal", () => {
  it("returns null when a turn has no token metrics", () => {
    expect(tokenTotal(null)).toBeNull();
  });

  it("includes zero-valued token metrics in the total", () => {
    expect(tokenTotal({
      turn_id: "turn-1",
      input_tokens: 0,
      output_tokens: 12,
      cache_read_tokens: 0,
      cache_write_tokens: null,
      tool_call_count: 0,
      outbox_write_count: 0,
      error_count: 0,
      updated_at: 1,
    })).toBe(12);
  });
});
