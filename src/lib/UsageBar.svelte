<script lang="ts">
  interface Props {
    label: string;
    subtitle: string;
    percentage: number;
    detail?: string;
  }

  let { label, subtitle, percentage, detail }: Props = $props();

  let displayPercent = $derived(Math.round(percentage));
</script>

<div class="usage-row">
  <div class="usage-left">
    <span class="usage-label">{label}</span>
    <span class="usage-subtitle">{subtitle}</span>
  </div>
  <div class="usage-right">
    <div class="bar-track">
      <div
        class="bar-fill"
        style="width: {Math.min(displayPercent, 100)}%;"
      ></div>
    </div>
    <span class="usage-percent">{detail || `${displayPercent}% used`}</span>
  </div>
</div>

<style>
  .usage-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 12px 0;
  }

  .usage-left {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 90px;
    flex-shrink: 0;
  }

  .usage-label {
    font-size: 13px;
    font-weight: 400;
    color: var(--text-primary);
  }

  .usage-subtitle {
    font-size: 11px;
    color: var(--text-secondary);
  }

  .usage-right {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: 1;
  }

  .bar-track {
    flex: 1;
    height: 8px;
    background: var(--bg-track);
    border-radius: 4px;
    overflow: hidden;
  }

  .bar-fill {
    height: 100%;
    border-radius: 4px;
    background: var(--accent-blue);
    transition: width 0.6s ease;
    min-width: 4px;
  }

  .usage-percent {
    font-size: 13px;
    color: var(--text-secondary);
    white-space: nowrap;
    min-width: 60px;
    text-align: right;
  }
</style>
