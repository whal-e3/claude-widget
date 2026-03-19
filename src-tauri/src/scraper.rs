use crate::models::UsageData;
use log::info;
use tauri::{AppHandle, Listener, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::oneshot;

const USAGE_URL: &str = "https://claude.ai/settings/usage";

pub struct WebViewScraper;

impl WebViewScraper {
    pub async fn scrape_usage(app: &AppHandle) -> Result<UsageData, String> {
        info!("Fallback: scraping usage via hidden WebView");

        // Create a hidden webview window
        let scraper_window = WebviewWindowBuilder::new(
            app,
            "scraper",
            WebviewUrl::External(USAGE_URL.parse().unwrap()),
        )
        .title("Scraper")
        .inner_size(800.0, 600.0)
        .visible(false)
        .build()
        .map_err(|e| format!("Failed to create scraper window: {}", e))?;

        // Wait for the page to load, then inject scraping JS
        let (tx, rx) = oneshot::channel::<Result<UsageData, String>>();
        let tx = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));

        let app_handle = app.clone();
        let tx_clone = tx.clone();

        // Listen for scraped data from the webview
        let handler_id = app.listen("usage-scraped", move |event| {
            if let Some(tx) = tx_clone.lock().unwrap().take() {
                let payload = event.payload();
                match serde_json::from_str::<UsageData>(payload) {
                    Ok(data) => {
                        let _ = tx.send(Ok(data));
                    }
                    Err(e) => {
                        let _ = tx.send(Err(format!("Failed to parse scraped data: {}", e)));
                    }
                }
            }
        });

        // Inject scraping script after a delay to let the page load
        let scraper_handle = scraper_window.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            let _ = scraper_handle.eval(&Self::scraping_script());
        });

        // Wait for result with timeout
        let result = tokio::time::timeout(tokio::time::Duration::from_secs(30), rx)
            .await
            .map_err(|_| "Scraping timed out after 30 seconds".to_string())?
            .map_err(|_| "Scraping channel closed".to_string())?;

        // Cleanup
        app_handle.unlisten(handler_id);
        let _ = scraper_window.close();

        result
    }

    fn scraping_script() -> String {
        r#"
        (function() {
            try {
                const models = [];

                // Strategy 1: Look for progress bars / usage indicators
                const progressElements = document.querySelectorAll('[role="progressbar"], .progress-bar, [class*="progress"], [class*="usage"]');

                // Strategy 2: Look for text containing model names
                const allText = document.body.innerText;
                const modelNames = ['opus', 'sonnet', 'haiku'];

                for (const modelName of modelNames) {
                    // Try to find percentage near model name mentions
                    const regex = new RegExp(modelName + '[\\s\\S]{0,100}?(\\d+(?:\\.\\d+)?)[\\s]*%', 'i');
                    const match = allText.match(regex);

                    if (match) {
                        models.push({
                            model_name: modelName,
                            utilization: parseFloat(match[1]) / 100.0,
                            messages_used: null,
                            messages_limit: null,
                            tokens_used: null,
                            cost: null
                        });
                    }
                }

                // Strategy 3: Look for data attributes
                if (models.length === 0) {
                    document.querySelectorAll('[data-model], [data-usage]').forEach(el => {
                        const name = el.getAttribute('data-model') || el.textContent.trim();
                        const usage = parseFloat(el.getAttribute('data-usage') || '0');
                        if (name && modelNames.some(m => name.toLowerCase().includes(m))) {
                            models.push({
                                model_name: name.toLowerCase(),
                                utilization: usage / 100.0,
                                messages_used: null,
                                messages_limit: null,
                                tokens_used: null,
                                cost: null
                            });
                        }
                    });
                }

                // Try to find reset time
                const resetRegex = /reset[s]?\s+(?:in\s+)?(\d+)\s*h(?:ours?)?\s*(\d+)?\s*m?/i;
                const resetMatch = allText.match(resetRegex);
                let resetAt = new Date(Date.now() + 5 * 60 * 60 * 1000).toISOString(); // default 5h
                if (resetMatch) {
                    const hours = parseInt(resetMatch[1]) || 0;
                    const minutes = parseInt(resetMatch[2]) || 0;
                    resetAt = new Date(Date.now() + (hours * 60 + minutes) * 60 * 1000).toISOString();
                }

                const result = {
                    models: models,
                    reset_at: resetAt
                };

                window.__TAURI__.event.emit('usage-scraped', result);
            } catch(e) {
                window.__TAURI__.event.emit('usage-scraped', {
                    models: [],
                    reset_at: new Date(Date.now() + 5 * 60 * 60 * 1000).toISOString()
                });
            }
        })();
        "#.to_string()
    }
}
