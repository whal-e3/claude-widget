<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  let step: "start" | "waiting" = $state("start");
  let status: string = $state("");
  let loading: boolean = $state(false);

  async function openBrowser() {
    await invoke("open_login_browser");
    step = "waiting";
    status = "";
  }

  async function captureSession() {
    loading = true;
    status = "Reading session from browser...";
    try {
      const result = await invoke<string>("capture_browser_cookies");
      status = result;
    } catch (e: any) {
      status = String(e);
    } finally {
      loading = false;
    }
  }
</script>

<div class="login-container">
  {#if step === "start"}
    <div class="login-icon">C</div>
    <p class="login-text">Sign in to view your Claude usage</p>
    <button class="login-btn" onclick={openBrowser}>
      Login to Claude
    </button>
  {:else}
    <p class="login-text">
      Log into claude.ai in your browser.<br />
      When done, click below.
    </p>
    <button class="login-btn" onclick={captureSession} disabled={loading}>
      {loading ? "Reading..." : "I'm Logged In"}
    </button>
    <button class="login-btn secondary" onclick={openBrowser}>
      Re-open Browser
    </button>
  {/if}

  {#if status}
    <p class="status" class:error={status.includes("Could not") || status.includes("Error") || status.includes("not found") || status.includes("No ")}>{status}</p>
  {/if}
</div>

<style>
  .login-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 8px;
    padding: 16px;
  }

  .login-icon {
    width: 40px;
    height: 40px;
    border-radius: 10px;
    background: linear-gradient(135deg, var(--accent), #9b6bf0);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 20px;
    font-weight: 700;
    color: #fff;
  }

  .login-text {
    color: var(--text-secondary);
    font-size: 12px;
    text-align: center;
    line-height: 1.5;
  }

  .login-btn {
    padding: 8px 20px;
    background: var(--accent);
    color: #fff;
    border-radius: 8px;
    font-size: 13px;
    font-weight: 500;
    transition: all 0.15s;
    width: 100%;
  }

  .login-btn.secondary {
    background: var(--bg-secondary);
    color: var(--text-secondary);
  }

  .login-btn:hover:not(:disabled) {
    opacity: 0.85;
  }

  .login-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .status {
    font-size: 10px;
    color: var(--text-secondary);
    text-align: center;
    max-width: 100%;
    word-break: break-word;
  }

  .status.error {
    color: var(--red);
  }
</style>
