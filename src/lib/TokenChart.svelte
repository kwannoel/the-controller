<script lang="ts">
  interface Props {
    dataPoints: {
      timestamp: string;
      input_tokens: number;
      output_tokens: number;
      cache_read_tokens: number;
      cache_write_tokens: number;
    }[];
  }

  let { dataPoints }: Props = $props();

  // Chart dimensions
  const chartHeight = 80;
  const barGap = 2;
  const minBarWidth = 6;
  const maxBarWidth = 24;

  let bars = $derived.by(() => {
    if (dataPoints.length === 0) return [];

    const barWidth = Math.max(
      minBarWidth,
      Math.min(maxBarWidth, Math.floor(300 / dataPoints.length) - barGap)
    );

    const maxTokens = Math.max(
      ...dataPoints.map((d) => d.input_tokens + d.output_tokens)
    );
    if (maxTokens === 0) return [];

    const scale = chartHeight / maxTokens;

    return dataPoints.map((d, i) => {
      const inputH = d.input_tokens * scale;
      const outputH = d.output_tokens * scale;
      const x = i * (barWidth + barGap);
      return {
        x,
        width: barWidth,
        inputH,
        outputH,
        totalH: inputH + outputH,
        input_tokens: d.input_tokens,
        output_tokens: d.output_tokens,
      };
    });
  });

  let chartWidth = $derived(
    bars.length > 0
      ? bars[bars.length - 1].x + bars[bars.length - 1].width
      : 0
  );

  let totalInput = $derived(
    dataPoints.reduce((sum, d) => sum + d.input_tokens, 0)
  );
  let totalOutput = $derived(
    dataPoints.reduce((sum, d) => sum + d.output_tokens, 0)
  );

  function formatTokens(n: number): string {
    if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + "M";
    if (n >= 1_000) return (n / 1_000).toFixed(1) + "k";
    return n.toString();
  }
</script>

{#if dataPoints.length > 0}
  <div class="token-chart">
    <div class="token-header">
      <span class="label">TOKENS</span>
      <span class="totals">
        <span class="input-badge">{formatTokens(totalInput)} in</span>
        <span class="output-badge">{formatTokens(totalOutput)} out</span>
      </span>
    </div>
    <div class="chart-container">
      <svg
        width={chartWidth}
        height={chartHeight}
        viewBox="0 0 {chartWidth} {chartHeight}"
      >
        {#each bars as bar, i (i)}
          <!-- Output tokens (bottom) -->
          <rect
            x={bar.x}
            y={chartHeight - bar.outputH}
            width={bar.width}
            height={bar.outputH}
            fill="#4a9e6e"
            rx="1"
          />
          <!-- Input tokens (stacked on top) -->
          <rect
            x={bar.x}
            y={chartHeight - bar.totalH}
            width={bar.width}
            height={bar.inputH}
            fill="#5a9bcf"
            rx="1"
          />
          <title>Turn {i + 1}: {formatTokens(bar.input_tokens)} in, {formatTokens(bar.output_tokens)} out</title>
        {/each}
      </svg>
    </div>
  </div>
{/if}

<style>
  .token-chart {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .token-header {
    display: flex;
    align-items: baseline;
    gap: 12px;
  }

  .label {
    color: var(--text-secondary);
    font-weight: 600;
    font-size: 15px;
    letter-spacing: 0.75px;
    flex-shrink: 0;
    width: 63px;
  }

  .totals {
    display: flex;
    gap: 10px;
    font-size: 14px;
  }

  .input-badge {
    color: var(--text-emphasis);
  }

  .output-badge {
    color: var(--status-idle);
  }

  .chart-container {
    padding-left: 75px;
    overflow-x: auto;
    scrollbar-width: none;
  }

  .chart-container::-webkit-scrollbar {
    display: none;
  }

  svg {
    display: block;
  }
</style>
