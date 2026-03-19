use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageData {
    pub models: Vec<ModelUsage>,
    pub reset_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUsage {
    pub model_name: String,
    pub utilization: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages_used: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages_limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_used: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthCredentials {
    pub session_key: Option<String>,
    pub bearer_token: Option<String>,
    pub cookies: Vec<CookieEntry>,
    pub organization_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookieEntry {
    pub name: String,
    pub value: String,
    pub domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub timestamp: DateTime<Utc>,
    pub data: UsageData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub corner_position: CornerPosition,
    pub poll_interval_secs: u64,
    pub auto_hide: bool,
    pub working_endpoint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CornerPosition {
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            corner_position: CornerPosition::BottomRight,
            poll_interval_secs: 60,
            auto_hide: true,
            working_endpoint: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerState {
    pub consecutive_failures: u32,
    pub fallback_active: bool,
    pub threshold: u32,
}

impl Default for CircuitBreakerState {
    fn default() -> Self {
        Self {
            consecutive_failures: 0,
            fallback_active: false,
            threshold: 3,
        }
    }
}

impl CircuitBreakerState {
    pub fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        if self.consecutive_failures >= self.threshold {
            self.fallback_active = true;
        }
    }

    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.fallback_active = false;
    }
}
