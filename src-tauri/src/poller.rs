use crate::api::ApiClient;
use crate::models::{AppConfig, CircuitBreakerState, HistoryEntry, UsageData};
use crate::scraper::WebViewScraper;
use crate::session;
use chrono::Utc;
use log::{error, info, warn};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use tokio::time::{self, Duration};

pub struct Poller {
    app: AppHandle,
    pub api_client: ApiClient,
    circuit_breaker: Arc<Mutex<CircuitBreakerState>>,
    config: Arc<Mutex<AppConfig>>,
    history_path: PathBuf,
    last_data: Arc<Mutex<Option<UsageData>>>,
}

impl Poller {
    pub fn new(app: AppHandle) -> Self {
        let history_path = app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("usage_history.json");

        Self {
            app,
            api_client: ApiClient::new(),
            circuit_breaker: Arc::new(Mutex::new(CircuitBreakerState::default())),
            config: Arc::new(Mutex::new(AppConfig::default())),
            history_path,
            last_data: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn start_polling(self: Arc<Self>) {
        info!("Starting usage poller");

        // Try initial fetch
        self.poll_once().await;

        // Re-emit cached data after 3s so frontend catches it after mounting
        let self_clone = self.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(3)).await;
            let cached = self_clone.last_data.lock().await;
            if let Some(data) = cached.as_ref() {
                info!("Re-emitting cached data to frontend");
                if let Some(window) = self_clone.app.get_webview_window("main") {
                    let _ = window.emit("usage-updated", data);
                } else {
                    let _ = self_clone.app.emit("usage-updated", data);
                }
            }
        });

        // Periodic polling
        let poll_interval = {
            let config = self.config.lock().await;
            config.poll_interval_secs
        };

        let mut interval = time::interval(Duration::from_secs(poll_interval));
        interval.tick().await;

        loop {
            interval.tick().await;

            match session::load_credentials() {
                Ok(Some(_)) => {
                    self.poll_once().await;
                }
                Ok(None) => {
                    time::sleep(Duration::from_secs(10)).await;
                }
                Err(e) => {
                    error!("Failed to load credentials: {}", e);
                    time::sleep(Duration::from_secs(30)).await;
                }
            }
        }
    }

    pub async fn poll_once(&self) {
        let credentials = match session::load_credentials() {
            Ok(Some(creds)) => {
                log::debug!(
                    "Loaded credentials: session_key={}, cookies={}, org_id={}",
                    creds.session_key.is_some(),
                    creds.cookies.len(),
                    creds.organization_id.is_some(),
                );
                creds
            }
            Ok(None) => {
                info!("No credentials available, skipping poll");
                return;
            }
            Err(e) => {
                error!("Failed to load credentials: {}", e);
                return;
            }
        };

        let mut cb = self.circuit_breaker.lock().await;

        if cb.fallback_active {
            info!("Using WebView scraper fallback...");
            let _ = self.app.emit("fallback-active", true);
            match WebViewScraper::scrape_usage(&self.app).await {
                Ok(data) => {
                    info!("Scraper fallback succeeded: {} models", data.models.len());
                    self.handle_usage_data(data).await;
                    cb.record_success();
                    let _ = self.app.emit("fallback-active", false);
                }
                Err(e) => {
                    error!("Scraper fallback also failed: {}", e);
                    let _ = self.app.emit("fetch-error", e);
                }
            }
        } else {
            let working_endpoint = {
                let config = self.config.lock().await;
                config.working_endpoint.clone()
            };

            info!(
                "Fetching usage data via API (working_endpoint: {:?})...",
                working_endpoint
            );

            match self
                .api_client
                .fetch_usage(&credentials, working_endpoint.as_deref())
                .await
            {
                Ok((data, endpoint)) => {
                    info!(
                        "API fetch succeeded via {}: {} models found",
                        endpoint,
                        data.models.len()
                    );
                    for m in &data.models {
                        info!("  {} = {:.0}%", m.model_name, m.utilization * 100.0);
                    }
                    cb.record_success();

                    {
                        let mut config = self.config.lock().await;
                        config.working_endpoint = Some(endpoint);
                    }

                    self.handle_usage_data(data).await;
                }
                Err(e) => {
                    if e.contains("Session expired") {
                        warn!("Session expired, notifying frontend");
                        let _ = self.app.emit("session-expired", ());
                        return;
                    }

                    cb.record_failure();
                    warn!(
                        "API fetch failed ({}/{}): {}",
                        cb.consecutive_failures, cb.threshold, e
                    );

                    if cb.fallback_active {
                        info!("Circuit breaker tripped, switching to WebView fallback");
                        let _ = self.app.emit("fallback-active", true);
                    }

                    let _ = self.app.emit("fetch-error", e);
                }
            }
        }
    }

    async fn handle_usage_data(&self, data: UsageData) {
        // Cache for instant retrieval via get_cached_usage
        {
            let mut cached = self.last_data.lock().await;
            *cached = Some(data.clone());
        }

        // Emit to frontend
        if let Some(window) = self.app.get_webview_window("main") {
            let _ = window.emit("usage-updated", &data);
        } else {
            let _ = self.app.emit("usage-updated", &data);
        }

        if let Err(e) = self.save_to_history(&data).await {
            warn!("Failed to save history: {}", e);
        }
    }

    async fn save_to_history(&self, data: &UsageData) -> Result<(), String> {
        let entry = HistoryEntry {
            timestamp: Utc::now(),
            data: data.clone(),
        };

        let mut history: Vec<HistoryEntry> = if self.history_path.exists() {
            let content =
                std::fs::read_to_string(&self.history_path).map_err(|e| e.to_string())?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Vec::new()
        };

        history.push(entry);

        let seven_days_ago = Utc::now() - chrono::Duration::days(7);
        history.retain(|h| h.timestamp > seven_days_ago);

        if let Some(parent) = self.history_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let json = serde_json::to_string_pretty(&history).map_err(|e| e.to_string())?;
        std::fs::write(&self.history_path, json).map_err(|e| e.to_string())?;

        Ok(())
    }
}

// Store the poller in Tauri state so commands can access it
pub struct PollerState(pub Arc<Poller>);

#[tauri::command(async)]
pub async fn get_usage_history(app: AppHandle) -> Result<Vec<HistoryEntry>, String> {
    let history_path = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("usage_history.json");

    if !history_path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&history_path).map_err(|e| e.to_string())?;
    let history: Vec<HistoryEntry> = serde_json::from_str(&content).unwrap_or_default();
    Ok(history)
}

#[tauri::command(async)]
pub async fn force_refresh(state: tauri::State<'_, PollerState>) -> Result<(), String> {
    info!("Force refresh triggered via command");
    state.0.poll_once().await;
    Ok(())
}

/// Instant command to get cached usage data (no API call)
#[tauri::command(async)]
pub async fn get_cached_usage(state: tauri::State<'_, PollerState>) -> Result<Option<UsageData>, String> {
    let cached = state.0.last_data.lock().await;
    Ok(cached.clone())
}

/// Direct command to get current usage data (makes API call)
#[tauri::command(async)]
pub async fn get_current_usage(state: tauri::State<'_, PollerState>) -> Result<Option<UsageData>, String> {
    info!("get_current_usage called");
    let credentials = match session::load_credentials() {
        Ok(Some(creds)) => creds,
        Ok(None) => return Ok(None),
        Err(e) => return Err(e),
    };

    let working_endpoint = None::<&str>;
    match state.0.api_client.fetch_usage(&credentials, working_endpoint).await {
        Ok((data, _)) => Ok(Some(data)),
        Err(e) => Err(e),
    }
}
