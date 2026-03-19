<script lang="ts">
  import { onMount, onDestroy } from "svelte";

  interface Props {
    resetAt: string;
  }

  let { resetAt }: Props = $props();
  let timeLeft: string = $state("");
  let intervalId: ReturnType<typeof setInterval>;

  function updateTimer() {
    const now = Date.now();
    const target = new Date(resetAt).getTime();
    const diff = target - now;

    if (diff <= 0) {
      timeLeft = "Resetting...";
      return;
    }

    const hours = Math.floor(diff / (1000 * 60 * 60));
    const minutes = Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60));

    if (hours > 0) {
      timeLeft = `${hours}h ${minutes}m`;
    } else {
      timeLeft = `${minutes}m`;
    }
  }

  onMount(() => {
    updateTimer();
    // Recalculate from absolute time each tick (no drift)
    intervalId = setInterval(updateTimer, 60_000);
  });

  onDestroy(() => {
    if (intervalId) clearInterval(intervalId);
  });
</script>

<div class="timer">
  <span class="timer-label">Reset in</span>
  <span class="timer-value">{timeLeft}</span>
</div>

<style>
  .timer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 0;
    border-top: 1px solid var(--border);
  }

  .timer-label {
    font-size: 11px;
    color: var(--text-secondary);
  }

  .timer-value {
    font-size: 13px;
    font-weight: 600;
    color: var(--accent);
    font-variant-numeric: tabular-nums;
  }
</style>
