<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  let step: "start" | "waiting" | "manual" = $state("start");
  let status: string = $state("");
  let loading: boolean = $state(false);
  let cookieInput: string = $state("");

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
      const err = String(e);
      status = err;
      // If auto-capture fails, show manual paste option
      if (err.includes("Could not") || err.includes("No ") || err.includes("encrypted")) {
        step = "manual";
        status = "Auto-capture failed. Paste your session cookie manually.";
      }
    } finally {
      loading = false;
    }
  }

  async function saveManualCookie() {
    if (!cookieInput.trim()) return;
    loading = true;
    status = "Verifying...";
    try {
      await invoke("save_session_cookie", { cookieValue: cookieInput.trim() });
      status = "Connected!";
    } catch (e) {
      status = `Error: ${e}`;
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
  {:else if step === "waiting"}
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
  {:else}
    <p class="login-text">
      1. Open claude.ai in your browser (F12 → Console)<br />
      2. Type: <code>document.cookie</code><br />
      3. Find <code>sessionKey=xxx</code> and copy the xxx part
    </p>
    <textarea
      class="cookie-input"
      placeholder="Paste sessionKey value here..."
      bind:value={cookieInput}
      rows="3"
    ></textarea>
    <button
      class="login-btn"
      onclick={saveManualCookie}
      disabled={loading || !cookieInput.trim()}
    >
      {loading ? "Verifying..." : "Connect"}
    </button>
    <button class="cookie-link" onclick={() => (step = "waiting")}>
      Try auto-capture again
    </button>
  {/if}

  {#if status}
    <p class="status" class:error={status.includes("Error") || status.includes("failed") || status.includes("Could not")}>{status}</p>
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
    background: linear-gradient(135deg, #5b8def, #9b6bf0);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 20px;
    font-weight: 700;
    color: #fff;
  }

  .login-text {
    color: var(--text-secondary);
    font-size: 11px;
    text-align: center;
    line-height: 1.6;
  }

  .login-btn {
    padding: 8px 20px;
    background: #5b8def;
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

  .cookie-link {
    font-size: 10px;
    color: var(--text-muted);
    text-decoration: underline;
    padding: 2px;
  }

  .cookie-link:hover {
    color: var(--text-secondary);
  }

  .cookie-input {
    width: 100%;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--text-primary);
    font-size: 11px;
    font-family: monospace;
    padding: 6px 8px;
    resize: none;
  }

  .cookie-input:focus {
    outline: 1px solid #5b8def;
    border-color: #5b8def;
  }

  code {
    background: var(--bg-secondary);
    padding: 1px 4px;
    border-radius: 3px;
    font-size: 10px;
    color: #5b8def;
  }

  .status {
    font-size: 10px;
    color: var(--text-secondary);
    text-align: center;
    max-width: 100%;
    word-break: break-word;
  }

  .status.error {
    color: #f87171;
  }
</style>
