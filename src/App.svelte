<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import UsageBar from "./lib/UsageBar.svelte";
  import Login from "./lib/Login.svelte";
  import type { UsageData, AppState } from "./lib/types.ts";
  import "./app.css";

  let appState: AppState = $state("loading");
  let usageData: UsageData | null = $state(null);
  let errorMessage: string = $state("");
  let lastUpdated: string = $state("");
  let refreshInterval: ReturnType<typeof setInterval>;

  onMount(async () => {
    try {
      const hasSession = await invoke<boolean>("check_session");
      appState = hasSession ? "connected" : "login";
    } catch {
      appState = "login";
    }

    await listen<UsageData>("usage-updated", (event) => {
      usageData = event.payload;
      appState = "connected";
      errorMessage = "";
      updateLastUpdated();
    });

    await listen("auth-success", async () => {
      appState = "connected";
      setTimeout(() => fetchUsageDirectly(), 1000);
    });

    await listen<string>("auth-error", (event) => {
      errorMessage = event.payload;
      appState = "error";
    });

    await listen("session-expired", () => {
      appState = "login";
      usageData = null;
    });

    await listen<string>("fetch-error", (event) => {
      errorMessage = event.payload;
    });

    // Always try to load data after a short delay (poller needs ~2s on startup)
    setTimeout(async () => {
      console.log("[widget] Trying get_cached_usage...");
      try {
        const cached = await invoke<UsageData | null>("get_cached_usage");
        console.log("[widget] get_cached_usage result:", cached);
        if (cached) {
          usageData = cached;
          appState = "connected";
          updateLastUpdated();
          return;
        }
      } catch (e) {
        console.error("[widget] get_cached_usage error:", e);
      }
      console.log("[widget] Trying get_current_usage...");
      const ok = await fetchUsageDirectly();
      console.log("[widget] fetchUsageDirectly result:", ok);
    }, 2000);

    // Retry at 6s if still no data
    setTimeout(async () => {
      if (!usageData) {
        console.log("[widget] Retry: fetching usage...");
        await fetchUsageDirectly();
      }
    }, 6000);

    // Auto-refresh every 60 seconds
    refreshInterval = setInterval(() => {
      if (appState === "connected") {
        fetchUsageDirectly();
      }
    }, 60_000);

    // Draggable title bar
    const titleBar = document.getElementById("title-bar");
    if (titleBar) {
      titleBar.addEventListener("mousedown", () => {
        getCurrentWindow().startDragging();
      });
    }
  });

  onDestroy(() => {
    if (refreshInterval) clearInterval(refreshInterval);
  });

  async function fetchUsageDirectly(): Promise<boolean> {
    try {
      const data = await invoke<UsageData | null>("get_current_usage");
      if (data) {
        usageData = data;
        appState = "connected";
        errorMessage = "";
        updateLastUpdated();
        return true;
      }
      return false;
    } catch (e: any) {
      errorMessage = String(e);
      return false;
    }
  }

  function updateLastUpdated() {
    lastUpdated = "less than a minute ago";
    // Update the text over time
    let seconds = 0;
    const timer = setInterval(() => {
      seconds += 60;
      const mins = Math.floor(seconds / 60);
      if (mins < 1) lastUpdated = "less than a minute ago";
      else if (mins === 1) lastUpdated = "1 minute ago";
      else lastUpdated = `${mins} minutes ago`;
    }, 60_000);
    // Clear previous timer on next update
    setTimeout(() => clearInterval(timer), 600_000);
  }

  function formatResetTime(resetAt: string): string {
    const now = Date.now();
    const target = new Date(resetAt).getTime();
    const diff = target - now;

    if (diff <= 0) return "Resetting...";

    const hours = Math.floor(diff / (1000 * 60 * 60));
    const minutes = Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60));

    if (hours > 24) {
      const date = new Date(resetAt);
      const days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
      const day = days[date.getDay()];
      const h = date.getHours();
      const ampm = h >= 12 ? "PM" : "AM";
      const h12 = h % 12 || 12;
      const min = date.getMinutes().toString().padStart(2, "0");
      return `Resets ${day} ${h12}:${min} ${ampm}`;
    }

    if (hours > 0) return `Resets in ${hours} hr ${minutes} min`;
    return `Resets in ${minutes} min`;
  }

  // Derive display data from the API response
  let fiveHour = $derived(usageData?.models.find(m => m.model_name === "5-Hour"));
  let sevenDay = $derived(usageData?.models.find(m => m.model_name === "7-Day"));
  let sonnet = $derived(usageData?.models.find(m => m.model_name === "Sonnet"));
  let opus = $derived(usageData?.models.find(m => m.model_name === "Opus"));
  let credits = $derived(usageData?.models.find(m => m.model_name === "Credits"));
</script>

<div class="widget">
  <div class="title-bar" id="title-bar">
    <span class="title">Claude Usage</span>
    <button class="refresh-btn" onclick={fetchUsageDirectly} title="Refresh">&#x21bb;</button>
  </div>

  <div class="content">
    {#if appState === "loading"}
      <div class="center-message">Loading...</div>
    {:else if appState === "login"}
      <Login />
    {:else if appState === "connected" && usageData}
      <div class="section">
        <h2 class="section-title">Plan usage limits</h2>

        {#if fiveHour}
          <UsageBar
            label="Current session"
            subtitle={formatResetTime(usageData.reset_at)}
            percentage={fiveHour.utilization * 100}
          />
        {/if}
      </div>

      <div class="divider"></div>

      <div class="section">
        <h2 class="section-title">Weekly limits</h2>

        {#if sevenDay}
          <UsageBar
            label="All models"
            subtitle={formatResetTime(usageData.reset_at)}
            percentage={sevenDay.utilization * 100}
          />
        {/if}

        {#if sonnet}
          <UsageBar
            label="Sonnet only"
            subtitle={formatResetTime(usageData.reset_at)}
            percentage={sonnet.utilization * 100}
          />
        {/if}

        {#if opus}
          <UsageBar
            label="Opus only"
            subtitle={formatResetTime(usageData.reset_at)}
            percentage={opus.utilization * 100}
          />
        {/if}
      </div>

      {#if credits}
        <div class="divider"></div>
        <div class="section">
          <UsageBar
            label="Extra credits"
            subtitle={credits.messages_used && credits.messages_limit
              ? `${credits.messages_used} / ${credits.messages_limit}`
              : ""}
            percentage={credits.utilization * 100}
          />
        </div>
      {/if}

      <div class="footer">
        <span>Last updated: {lastUpdated}</span>
        <button class="refresh-icon" onclick={fetchUsageDirectly} title="Refresh">&#x21bb;</button>
      </div>

      {#if errorMessage}
        <div class="error-banner">{errorMessage}</div>
      {/if}
    {:else if appState === "error"}
      <div class="center-message error">
        <p>{errorMessage}</p>
        <button class="retry-btn" onclick={() => { appState = "login"; }}>Try Again</button>
      </div>
    {:else}
      <div class="center-message">Fetching usage data...</div>
    {/if}
  </div>
</div>

<style>
  .widget {
    width: 100%;
    height: 100%;
    background: var(--bg-primary);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .title-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 16px;
    cursor: grab;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .title-bar:active {
    cursor: grabbing;
  }

  .title {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary);
    letter-spacing: 0.3px;
  }

  .refresh-btn {
    font-size: 14px;
    color: var(--text-secondary);
    padding: 2px 4px;
    border-radius: 4px;
  }

  .refresh-btn:hover {
    color: var(--text-primary);
    background: var(--bg-secondary);
  }

  .content {
    flex: 1;
    padding: 4px 20px 12px;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
  }

  .section {
    padding: 4px 0;
  }

  .section-title {
    font-size: 15px;
    font-weight: 600;
    color: var(--text-primary);
    padding: 12px 0 0;
  }

  .divider {
    height: 1px;
    background: var(--border);
    margin: 4px 0;
  }

  .footer {
    display: flex;
    align-items: center;
    gap: 6px;
    padding-top: 12px;
    font-size: 11px;
    color: var(--text-muted);
  }

  .refresh-icon {
    font-size: 12px;
    color: var(--text-muted);
    padding: 1px;
  }

  .refresh-icon:hover {
    color: var(--text-secondary);
  }

  .center-message {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--text-secondary);
    font-size: 13px;
    gap: 8px;
  }

  .center-message.error {
    color: #f87171;
  }

  .error-banner {
    font-size: 11px;
    color: #f87171;
    padding: 4px 8px;
    background: rgba(248, 113, 113, 0.1);
    border-radius: 4px;
    text-align: center;
    margin-top: 8px;
  }

  .retry-btn {
    padding: 6px 16px;
    background: var(--accent-blue);
    color: #fff;
    border-radius: 6px;
    font-size: 12px;
    font-weight: 500;
  }

  .retry-btn:hover {
    opacity: 0.85;
  }
</style>
