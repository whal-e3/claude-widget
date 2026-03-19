use crate::models::{AuthCredentials, CookieEntry};
use crate::session;
use log::info;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

const CLAUDE_URL: &str = "https://claude.ai";
const BROWSER_UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// Open the system browser for login.
/// Embedded WebViews are blocked by both Cloudflare and Google OAuth,
/// so we must use the user's real browser.
#[tauri::command(async)]
pub async fn open_login_browser() -> Result<(), String> {
    open::that(CLAUDE_URL).map_err(|e| format!("Failed to open browser: {}", e))?;
    info!("Opened system browser for Claude login");
    Ok(())
}

/// Automatically read session cookies from the user's browser.
/// Supports Firefox (including Snap) and Chromium-based browsers on Linux.
#[tauri::command(async)]
pub async fn capture_browser_cookies(app: AppHandle) -> Result<String, String> {
    info!("Attempting to read cookies from browser...");

    let (cookies, browser_name) = read_firefox_cookies()
        .map(|c| (c, "Firefox"))
        .or_else(|firefox_err| {
            info!("Firefox: {}", firefox_err);
            read_chromium_cookies().map(|c| (c, "Chromium"))
        })
        .map_err(|chromium_err| {
            format!(
                "Could not read cookies from any browser. Make sure you are logged into claude.ai.\nChromium: {}",
                chromium_err
            )
        })?;

    if cookies.is_empty() {
        return Err("No claude.ai cookies found. Please log into claude.ai in your browser first.".to_string());
    }

    info!("Found {} claude.ai cookies from {}", cookies.len(), browser_name);

    // Log available cookie names
    let names: Vec<&str> = cookies.iter().map(|c| c.name.as_str()).collect();
    log::debug!("Available cookies: {:?}", names);

    // Look for session key — Claude may use different cookie names
    let session_key = cookies
        .iter()
        .find(|c| {
            c.name == "sessionKey"
                || c.name == "__Secure-next-auth.session-token"
                || c.name == "cf_clearance"
                || c.name == "__ssid"
        })
        .map(|c| c.value.clone());

    // We don't require sessionKey — the full cookie set is sufficient for auth
    if cookies.is_empty() {
        return Err("No claude.ai cookies found. Please log in first.".to_string());
    }

    // Fetch org ID
    let org_id = fetch_org_id(&cookies).await;
    log::debug!("Organization ID: {:?}", org_id);

    let credentials = AuthCredentials {
        session_key,
        bearer_token: None,
        cookies,
        organization_id: org_id,
    };

    session::save_credentials(&credentials)?;
    info!("Credentials saved successfully");

    let _ = app.emit("auth-success", ());
    Ok(format!("Connected via {}", browser_name))
}

/// Read cookies from Firefox's cookies.sqlite (unencrypted on all platforms)
fn read_firefox_cookies() -> Result<Vec<CookieEntry>, String> {
    let home = dirs::home_dir().ok_or("No home directory")?;

    let mut candidates = vec![
        // Linux
        home.join("snap/firefox/common/.mozilla/firefox"),
        home.join(".mozilla/firefox"),
        home.join("snap/firefox/current/.mozilla/firefox"),
    ];

    // Windows
    if let Some(appdata) = dirs::data_dir() {
        candidates.push(appdata.join("Mozilla/Firefox/Profiles").parent().unwrap_or(&appdata).join("Mozilla/Firefox"));
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        candidates.push(PathBuf::from(&appdata).join("Mozilla/Firefox"));
    }

    let firefox_dir = candidates
        .iter()
        .find(|p| p.exists())
        .ok_or("Firefox not found at ~/.mozilla/firefox or ~/snap/firefox/")?
        .clone();

    info!("Found Firefox at {:?}", firefox_dir);

    let profile_dir = find_firefox_profile(&firefox_dir)?;
    let cookies_db = profile_dir.join("cookies.sqlite");

    if !cookies_db.exists() {
        return Err(format!("cookies.sqlite not found at {:?}", cookies_db));
    }

    info!("Reading cookies from {:?}", cookies_db);

    // Copy to avoid locking issues with Firefox
    let temp_db = std::env::temp_dir().join(format!("claude_widget_{}.sqlite", uuid::Uuid::new_v4()));
    std::fs::copy(&cookies_db, &temp_db)
        .map_err(|e| format!("Failed to copy cookies db: {}", e))?;

    let conn = rusqlite::Connection::open_with_flags(
        &temp_db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|e| format!("Failed to open cookies db: {}", e))?;

    let mut stmt = conn
        .prepare("SELECT name, value FROM moz_cookies WHERE host LIKE '%claude.ai'")
        .map_err(|e| format!("SQL error: {}", e))?;

    let cookies: Vec<CookieEntry> = stmt
        .query_map([], |row| {
            Ok(CookieEntry {
                name: row.get(0)?,
                value: row.get(1)?,
                domain: "claude.ai".to_string(),
            })
        })
        .map_err(|e| format!("Query error: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    let _ = std::fs::remove_file(&temp_db);

    if cookies.is_empty() {
        return Err("No claude.ai cookies in Firefox. Are you logged in?".to_string());
    }

    info!("Read {} cookies from Firefox", cookies.len());
    Ok(cookies)
}

fn find_firefox_profile(firefox_dir: &PathBuf) -> Result<PathBuf, String> {
    let profiles_ini = firefox_dir.join("profiles.ini");
    if profiles_ini.exists() {
        let content = std::fs::read_to_string(&profiles_ini).map_err(|e| e.to_string())?;

        let mut current_path: Option<String> = None;
        let mut default_path: Option<String> = None;
        let mut is_relative = true;

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("Path=") {
                current_path = Some(line.trim_start_matches("Path=").to_string());
            }
            if line.starts_with("IsRelative=") {
                is_relative = line.contains("1");
            }
            if line.starts_with("Default=1") {
                default_path = current_path.clone();
            }
        }

        let profile_path = default_path.or(current_path);
        if let Some(path) = profile_path {
            let full_path = if is_relative {
                firefox_dir.join(path)
            } else {
                PathBuf::from(path)
            };
            if full_path.exists() {
                return Ok(full_path);
            }
        }
    }

    // Fallback: any directory with cookies.sqlite
    if let Ok(entries) = std::fs::read_dir(firefox_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("cookies.sqlite").exists() {
                return Ok(path);
            }
        }
    }

    Err("No Firefox profile found".to_string())
}

/// Read cookies from Chromium-based browsers
fn read_chromium_cookies() -> Result<Vec<CookieEntry>, String> {
    let home = dirs::home_dir().ok_or("No home directory")?;

    let mut candidates = vec![
        // Linux
        home.join(".config/google-chrome/Default/Cookies"),
        home.join(".config/chromium/Default/Cookies"),
        home.join(".config/microsoft-edge/Default/Cookies"),
    ];

    // Windows
    if let Some(local_appdata) = dirs::data_local_dir() {
        candidates.push(local_appdata.join("Google/Chrome/User Data/Default/Cookies"));
        candidates.push(local_appdata.join("Google/Chrome/User Data/Default/Network/Cookies"));
        candidates.push(local_appdata.join("Microsoft/Edge/User Data/Default/Cookies"));
        candidates.push(local_appdata.join("Microsoft/Edge/User Data/Default/Network/Cookies"));
        candidates.push(local_appdata.join("BraveSoftware/Brave-Browser/User Data/Default/Cookies"));
        candidates.push(local_appdata.join("BraveSoftware/Brave-Browser/User Data/Default/Network/Cookies"));
    }

    for cookies_db in &candidates {
        if !cookies_db.exists() {
            continue;
        }

        let temp_db = std::env::temp_dir().join("claude_widget_chrome_cookies.sqlite");
        if std::fs::copy(cookies_db, &temp_db).is_err() {
            continue;
        }

        let conn = match rusqlite::Connection::open_with_flags(
            &temp_db,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        ) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut stmt = match conn.prepare("SELECT name, value FROM cookies WHERE host_key LIKE '%claude.ai'") {
            Ok(s) => s,
            Err(_) => continue,
        };

        let cookies: Vec<CookieEntry> = stmt
            .query_map([], |row| {
                Ok(CookieEntry {
                    name: row.get(0)?,
                    value: row.get(1)?,
                    domain: "claude.ai".to_string(),
                })
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).filter(|c| !c.value.is_empty()).collect())
            .unwrap_or_default();

        let _ = std::fs::remove_file(&temp_db);

        if !cookies.is_empty() {
            return Ok(cookies);
        }
    }

    Err("No Chromium browser with readable claude.ai cookies found".to_string())
}

async fn fetch_org_id(cookies: &[CookieEntry]) -> Option<String> {
    let cookie_str: String = cookies
        .iter()
        .map(|c| format!("{}={}", c.name, c.value))
        .collect::<Vec<_>>()
        .join("; ");

    let client = reqwest::Client::new();
    let resp = client
        .get("https://claude.ai/api/organizations")
        .header("Cookie", cookie_str)
        .header("User-Agent", BROWSER_UA)
        .send()
        .await
        .ok()?;

    let body: serde_json::Value = resp.json().await.ok()?;
    if let Some(orgs) = body.as_array() {
        orgs.first()
            .and_then(|o| o.get("uuid"))
            .and_then(|u| u.as_str())
            .map(|s| s.to_string())
    } else {
        None
    }
}

/// Manual session cookie input (fallback for Windows where cookies are encrypted)
#[tauri::command(async)]
pub async fn save_session_cookie(app: AppHandle, cookie_value: String) -> Result<bool, String> {
    let cookie_value = cookie_value.trim().to_string();
    if cookie_value.is_empty() {
        return Err("Cookie value is empty".to_string());
    }

    info!("Saving manual session cookie ({} chars)", cookie_value.len());

    let cookie_entry = CookieEntry {
        name: "sessionKey".to_string(),
        value: cookie_value,
        domain: "claude.ai".to_string(),
    };

    let org_id = fetch_org_id(&[cookie_entry.clone()]).await;

    let credentials = AuthCredentials {
        session_key: Some(cookie_entry.value.clone()),
        bearer_token: None,
        cookies: vec![cookie_entry],
        organization_id: org_id,
    };

    session::save_credentials(&credentials)?;
    let _ = app.emit("auth-success", ());
    Ok(true)
}

#[tauri::command(async)]
pub async fn check_session() -> Result<bool, String> {
    match session::load_credentials()? {
        Some(creds) => {
            let has_auth = creds.session_key.is_some()
                || creds.bearer_token.is_some()
                || !creds.cookies.is_empty();
            Ok(has_auth)
        }
        None => Ok(false),
    }
}

#[tauri::command(async)]
pub async fn clear_session() -> Result<(), String> {
    session::clear_credentials()
}
