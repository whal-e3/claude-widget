use crate::models::{AuthCredentials, ModelUsage, UsageData};
use chrono::{DateTime, Duration, Utc};
use log::{info, warn};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, COOKIE};
use serde_json::Value;

const CANDIDATE_ENDPOINTS: &[&str] = &[
    "https://claude.ai/api/organizations/{org_id}/usage",
    "https://claude.ai/api/settings/usage",
    "https://claude.ai/api/usage",
];

const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

pub struct ApiClient {
    client: reqwest::Client,
}

impl ApiClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(BROWSER_USER_AGENT)
            .build()
            .expect("Failed to create HTTP client");
        Self { client }
    }

    pub async fn fetch_usage(
        &self,
        credentials: &AuthCredentials,
        working_endpoint: Option<&str>,
    ) -> Result<(UsageData, String), String> {
        let headers = self.build_headers(credentials)?;

        // Try working endpoint first if we have one
        if let Some(endpoint) = working_endpoint {
            let url = self.resolve_endpoint(endpoint, credentials);
            match self.try_fetch(&url, &headers).await {
                Ok(data) => return Ok((data, endpoint.to_string())),
                Err(e) => warn!("Working endpoint failed: {} - {}", url, e),
            }
        }

        // Probe all candidate endpoints
        for endpoint_template in CANDIDATE_ENDPOINTS {
            let url = self.resolve_endpoint(endpoint_template, credentials);
            info!("Probing endpoint: {}", url);
            match self.try_fetch(&url, &headers).await {
                Ok(data) => {
                    info!("Found working endpoint: {}", endpoint_template);
                    return Ok((data, endpoint_template.to_string()));
                }
                Err(e) => {
                    warn!("Endpoint {} failed: {}", url, e);
                    continue;
                }
            }
        }

        Err("All API endpoints failed. Usage data unavailable.".to_string())
    }

    fn resolve_endpoint(&self, template: &str, credentials: &AuthCredentials) -> String {
        if let Some(org_id) = &credentials.organization_id {
            template.replace("{org_id}", org_id)
        } else {
            template.replace("/{org_id}", "").replace("{org_id}/", "")
        }
    }

    fn build_headers(&self, credentials: &AuthCredentials) -> Result<HeaderMap, String> {
        let mut headers = HeaderMap::new();

        // Add Bearer token if available
        if let Some(token) = &credentials.bearer_token {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", token))
                    .map_err(|e| e.to_string())?,
            );
        }

        // Add cookies
        if !credentials.cookies.is_empty() {
            let cookie_str: String = credentials
                .cookies
                .iter()
                .map(|c| format!("{}={}", c.name, c.value))
                .collect::<Vec<_>>()
                .join("; ");
            headers.insert(
                COOKIE,
                HeaderValue::from_str(&cookie_str).map_err(|e| e.to_string())?,
            );
        }

        // Add browser-like headers
        headers.insert(
            "Origin",
            HeaderValue::from_static("https://claude.ai"),
        );
        headers.insert(
            "Referer",
            HeaderValue::from_static("https://claude.ai/settings/usage"),
        );
        headers.insert(
            "Accept",
            HeaderValue::from_static("application/json"),
        );

        Ok(headers)
    }

    async fn try_fetch(&self, url: &str, headers: &HeaderMap) -> Result<UsageData, String> {
        let response = self
            .client
            .get(url)
            .headers(headers.clone())
            .send()
            .await
            .map_err(|e| format!("Network error: {}", e))?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err("Session expired".to_string());
        }
        if !status.is_success() {
            return Err(format!("HTTP {}", status));
        }

        let text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        log::debug!("Response from {}: {} bytes", url, text.len());

        let body: Value = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse JSON: {}", e))?;

        self.parse_usage_response(&body)
    }

    fn parse_usage_response(&self, body: &Value) -> Result<UsageData, String> {
        // Try multiple response formats since the API is undocumented
        // Format 1: Direct model usage array
        if let Some(models) = body.get("models").and_then(|m| m.as_array()) {
            return self.parse_models_array(models, body);
        }

        // Format 2: Utilization-based response
        if body.get("five_hour").is_some() || body.get("daily").is_some() {
            return self.parse_utilization_response(body);
        }

        // Format 3: Usage data nested under a key
        if let Some(usage) = body.get("usage") {
            if let Some(models) = usage.get("models").and_then(|m| m.as_array()) {
                return self.parse_models_array(models, usage);
            }
        }

        // Format 4: Try to extract any recognizable model data
        self.parse_generic_response(body)
    }

    fn parse_models_array(
        &self,
        models: &[Value],
        parent: &Value,
    ) -> Result<UsageData, String> {
        let model_usages: Vec<ModelUsage> = models
            .iter()
            .filter_map(|m| {
                let name = m
                    .get("model_name")
                    .or_else(|| m.get("name"))
                    .or_else(|| m.get("model"))
                    .and_then(|n| n.as_str())?;
                let utilization = m
                    .get("utilization")
                    .or_else(|| m.get("percentage"))
                    .or_else(|| m.get("usage_percent"))
                    .and_then(|u| u.as_f64())
                    .unwrap_or(0.0) as f32;

                Some(ModelUsage {
                    model_name: name.to_string(),
                    utilization,
                    messages_used: m
                        .get("messages_used")
                        .or_else(|| m.get("used"))
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32),
                    messages_limit: m
                        .get("messages_limit")
                        .or_else(|| m.get("limit"))
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32),
                    tokens_used: m
                        .get("tokens_used")
                        .or_else(|| m.get("tokens"))
                        .and_then(|v| v.as_u64()),
                    cost: m.get("cost").and_then(|v| v.as_f64()),
                })
            })
            .collect();

        let reset_at = parent
            .get("reset_at")
            .or_else(|| parent.get("resets_at"))
            .or_else(|| parent.get("next_reset"))
            .and_then(|r| r.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|| Utc::now() + Duration::hours(5));

        Ok(UsageData {
            models: model_usages,
            reset_at,
        })
    }

    fn parse_utilization_response(&self, body: &Value) -> Result<UsageData, String> {
        let mut models = Vec::new();

        // Parse claude.ai usage format:
        // five_hour: overall 5-hour usage
        // seven_day: overall 7-day usage
        // seven_day_sonnet, seven_day_opus, etc: per-model 7-day usage
        // extra_usage: extra credits usage

        // Add 5-hour overall usage
        if let Some(five_hour) = body.get("five_hour") {
            let utilization = five_hour.get("utilization").and_then(|u| u.as_f64()).unwrap_or(0.0) as f32;
            models.push(ModelUsage {
                model_name: "5-Hour".to_string(),
                utilization: utilization / 100.0,
                messages_used: None,
                messages_limit: None,
                tokens_used: None,
                cost: None,
            });
        }

        // Add 7-day overall usage
        if let Some(seven_day) = body.get("seven_day") {
            let utilization = seven_day.get("utilization").and_then(|u| u.as_f64()).unwrap_or(0.0) as f32;
            models.push(ModelUsage {
                model_name: "7-Day".to_string(),
                utilization: utilization / 100.0,
                messages_used: None,
                messages_limit: None,
                tokens_used: None,
                cost: None,
            });
        }

        // Add per-model 7-day usage (sonnet, opus, etc.)
        for (key, label) in &[
            ("seven_day_opus", "Opus"),
            ("seven_day_sonnet", "Sonnet"),
            ("seven_day_cowork", "Cowork"),
        ] {
            if let Some(val) = body.get(key) {
                if !val.is_null() {
                    let utilization = val.get("utilization").and_then(|u| u.as_f64()).unwrap_or(0.0) as f32;
                    models.push(ModelUsage {
                        model_name: label.to_string(),
                        utilization: utilization / 100.0,
                        messages_used: None,
                        messages_limit: None,
                        tokens_used: None,
                        cost: None,
                    });
                }
            }
        }

        // Add extra usage (credits)
        if let Some(extra) = body.get("extra_usage") {
            if !extra.is_null() && extra.get("is_enabled").and_then(|v| v.as_bool()).unwrap_or(false) {
                let used = extra.get("used_credits").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let limit = extra.get("monthly_limit").and_then(|v| v.as_f64()).unwrap_or(1.0);
                let utilization = extra.get("utilization").and_then(|u| u.as_f64()).unwrap_or(0.0) as f32;
                models.push(ModelUsage {
                    model_name: "Credits".to_string(),
                    utilization: utilization / 100.0,
                    messages_used: Some(used as u32),
                    messages_limit: Some(limit as u32),
                    tokens_used: None,
                    cost: Some(used),
                });
            }
        }

        // Get reset time from five_hour (most relevant for rate limiting)
        let reset_at = body
            .get("five_hour")
            .and_then(|fh| fh.get("resets_at"))
            .and_then(|r| r.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|| Utc::now() + Duration::hours(5));

        if models.is_empty() {
            return Err("No usage data found in response".to_string());
        }

        Ok(UsageData { models, reset_at })
    }

    fn parse_generic_response(&self, body: &Value) -> Result<UsageData, String> {
        // Last resort: scan for anything that looks like model usage
        let mut models = Vec::new();

        if let Some(obj) = body.as_object() {
            for (key, value) in obj {
                let key_lower = key.to_lowercase();
                if key_lower.contains("opus")
                    || key_lower.contains("sonnet")
                    || key_lower.contains("haiku")
                {
                    let utilization = value
                        .as_f64()
                        .or_else(|| value.get("utilization").and_then(|u| u.as_f64()))
                        .unwrap_or(0.0) as f32;

                    models.push(ModelUsage {
                        model_name: key.clone(),
                        utilization,
                        messages_used: None,
                        messages_limit: None,
                        tokens_used: None,
                        cost: None,
                    });
                }
            }
        }

        if models.is_empty() {
            return Err(format!(
                "Unable to parse usage data from response: {}",
                serde_json::to_string_pretty(body).unwrap_or_default()
            ));
        }

        Ok(UsageData {
            models,
            reset_at: Utc::now() + Duration::hours(5),
        })
    }
}
