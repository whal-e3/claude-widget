<script lang="ts">
  import type { HistoryEntry } from "./types.ts";

  interface Props {
    history: HistoryEntry[];
  }

  let { history }: Props = $props();

  const WIDTH = 268;
  const HEIGHT = 36;
  const PADDING = 2;

  let pathD = $derived.by(() => {
    if (history.length < 2) return "";

    // Average utilization across all models per data point
    const points = history.map((entry) => {
      const avg =
        entry.data.models.reduce((sum, m) => sum + m.utilization, 0) /
        Math.max(entry.data.models.length, 1);
      return avg;
    });

    // Take last 7 days worth of points (max ~2016 at 5min intervals, downsample)
    const maxPoints = 50;
    let sampled = points;
    if (points.length > maxPoints) {
      const step = Math.floor(points.length / maxPoints);
      sampled = [];
      for (let i = 0; i < points.length; i += step) {
        sampled.push(points[i]);
      }
    }

    const minVal = Math.min(...sampled, 0);
    const maxVal = Math.max(...sampled, 1);
    const range = maxVal - minVal || 1;

    const stepX = (WIDTH - PADDING * 2) / Math.max(sampled.length - 1, 1);

    const pathPoints = sampled.map((val, i) => {
      const x = PADDING + i * stepX;
      const y = HEIGHT - PADDING - ((val - minVal) / range) * (HEIGHT - PADDING * 2);
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    });

    return `M ${pathPoints.join(" L ")}`;
  });

  // Gradient fill path (closed)
  let fillD = $derived.by(() => {
    if (!pathD) return "";
    return `${pathD} L ${(WIDTH - PADDING).toFixed(1)},${(HEIGHT - PADDING).toFixed(1)} L ${PADDING.toFixed(1)},${(HEIGHT - PADDING).toFixed(1)} Z`;
  });
</script>

<div class="sparkline-container">
  <svg
    width={WIDTH}
    height={HEIGHT}
    viewBox="0 0 {WIDTH} {HEIGHT}"
    class="sparkline-svg"
  >
    <defs>
      <linearGradient id="sparkFill" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0%" stop-color="var(--accent)" stop-opacity="0.3" />
        <stop offset="100%" stop-color="var(--accent)" stop-opacity="0.02" />
      </linearGradient>
    </defs>

    {#if fillD}
      <path d={fillD} fill="url(#sparkFill)" />
    {/if}

    {#if pathD}
      <path
        d={pathD}
        fill="none"
        stroke="var(--accent)"
        stroke-width="1.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    {/if}
  </svg>
  <span class="sparkline-label">7-day trend</span>
</div>

<style>
  .sparkline-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding-top: 2px;
  }

  .sparkline-svg {
    display: block;
  }

  .sparkline-label {
    font-size: 9px;
    color: var(--text-muted);
    letter-spacing: 0.3px;
  }
</style>
