use crate::models::AuthCredentials;
use log::{info, warn};
use std::path::PathBuf;

fn credentials_file_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("claude-widget")
        .join("credentials.json")
}

pub fn save_credentials(credentials: &AuthCredentials) -> Result<(), String> {
    // Always save to file (reliable)
    save_to_file(credentials)?;

    // Also try keyring (best-effort)
    match save_to_keyring(credentials) {
        Ok(()) => info!("Credentials also saved to keyring"),
        Err(e) => warn!("Keyring save failed ({}), file backup exists", e),
    }

    Ok(())
}

pub fn load_credentials() -> Result<Option<AuthCredentials>, String> {
    // Try keyring first
    match load_from_keyring() {
        Ok(Some(creds)) => {
            info!("Credentials loaded from keyring ({} cookies)", creds.cookies.len());
            return Ok(Some(creds));
        }
        Ok(None) => {
            info!("No credentials in keyring");
        }
        Err(e) => {
            warn!("Keyring load failed: {}", e);
        }
    }

    // Fallback to file
    match load_from_file() {
        Ok(Some(creds)) => {
            info!("Credentials loaded from file ({} cookies)", creds.cookies.len());
            Ok(Some(creds))
        }
        Ok(None) => {
            info!("No credentials in file either");
            Ok(None)
        }
        Err(e) => {
            warn!("File load failed: {}", e);
            Ok(None)
        }
    }
}

pub fn clear_credentials() -> Result<(), String> {
    let _ = clear_keyring();
    let _ = clear_file();
    info!("Credentials cleared");
    Ok(())
}

// --- Keyring ---

const SERVICE_NAME: &str = "claude-usage-widget";
const CREDENTIALS_KEY: &str = "auth_credentials";

fn save_to_keyring(credentials: &AuthCredentials) -> Result<(), String> {
    let json = serde_json::to_string(credentials).map_err(|e| e.to_string())?;
    let entry = keyring::Entry::new(SERVICE_NAME, CREDENTIALS_KEY).map_err(|e| e.to_string())?;
    entry.set_password(&json).map_err(|e| e.to_string())
}

fn load_from_keyring() -> Result<Option<AuthCredentials>, String> {
    let entry = keyring::Entry::new(SERVICE_NAME, CREDENTIALS_KEY).map_err(|e| e.to_string())?;
    match entry.get_password() {
        Ok(json) => {
            let creds: AuthCredentials =
                serde_json::from_str(&json).map_err(|e| e.to_string())?;
            Ok(Some(creds))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

fn clear_keyring() -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE_NAME, CREDENTIALS_KEY).map_err(|e| e.to_string())?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

// --- File fallback ---

fn save_to_file(credentials: &AuthCredentials) -> Result<(), String> {
    let path = credentials_file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(credentials).map_err(|e| e.to_string())?;

    // Write with restricted permissions (owner-only: 0600)
    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)
            .map_err(|e| e.to_string())?;
        file.write_all(json.as_bytes()).map_err(|e| e.to_string())?;
    }

    #[cfg(not(unix))]
    {
        std::fs::write(&path, json).map_err(|e| e.to_string())?;
    }

    info!("Credentials saved to file");
    Ok(())
}

fn load_from_file() -> Result<Option<AuthCredentials>, String> {
    let path = credentials_file_path();
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let creds: AuthCredentials = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    info!("Credentials loaded from file: {:?}", path);
    Ok(Some(creds))
}

fn clear_file() -> Result<(), String> {
    let path = credentials_file_path();
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
