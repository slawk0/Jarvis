use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use tauri::Manager;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PangolinConfig {
    pub api_url: String,
    pub org_id: Option<String>,
}

fn get_pangolin_config_path(app_handle: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let mut path = app_handle.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&path).map_err(|e| format!("Cannot create configuration directory: {}", e))?;
    path.push("pangolin_config.json");
    Ok(path)
}

#[tauri::command]
pub fn get_pangolin_config(app_handle: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let path = get_pangolin_config_path(&app_handle)?;
    let config = if path.exists() {
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str::<PangolinConfig>(&content).unwrap_or_else(|_| PangolinConfig {
            api_url: "https://api.pangolin.net".to_string(),
            org_id: None,
        })
    } else {
        PangolinConfig {
            api_url: "https://api.pangolin.net".to_string(),
            org_id: None,
        }
    };
    
    // Check if API Key is in keyring
    let keyring_service = "JarvisPangolin";
    let keyring_entry = keyring::Entry::new(keyring_service, "api_key").ok();
    let has_api_key = keyring_entry.and_then(|e| e.get_password().ok()).is_some();
    
    Ok(serde_json::json!({
        "api_url": config.api_url,
        "org_id": config.org_id,
        "has_api_key": has_api_key
    }))
}

async fn check_health(client: &reqwest::Client, url: &str) -> bool {
    let test_url = format!("{}/v1", url);
    if let Ok(res) = client.get(&test_url).send().await {
        if res.status().is_success() {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                return json.get("message").and_then(|m| m.as_str()) == Some("Healthy");
            }
        }
    }
    false
}

#[tauri::command]
pub async fn save_pangolin_config(
    app_handle: tauri::AppHandle,
    api_url: String,
    org_id: Option<String>,
    api_key: Option<String>,
) -> Result<(), String> {
    let mut normalized_url = api_url.trim().trim_end_matches('/').to_string();
    
    // Validate/probe URL health with a 3-second timeout
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let mut health_ok = false;
    if !normalized_url.ends_with("/api") {
        if check_health(&client, &normalized_url).await {
            health_ok = true;
        } else {
            let alternative_url = format!("{}/api", normalized_url);
            if check_health(&client, &alternative_url).await {
                normalized_url = alternative_url;
                health_ok = true;
            }
        }
    } else {
        if check_health(&client, &normalized_url).await {
            health_ok = true;
        }
    }

    if !health_ok {
        return Err(format!(
            "Brak odpowiedzi z serwera Pangolin pod adresem: {}. Upewnij się, że URL jest poprawny.",
            api_url
        ));
    }

    let path = get_pangolin_config_path(&app_handle)?;
    let config = PangolinConfig { api_url: normalized_url, org_id };
    let content = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(path, content).map_err(|e| e.to_string())?;
    
    if let Some(key) = api_key {
        if !key.is_empty() {
            let keyring_service = "JarvisPangolin";
            let entry = keyring::Entry::new(keyring_service, "api_key")
                .map_err(|e| format!("Keyring init error: {}", e))?;
            entry.set_password(&key).map_err(|e| format!("Keyring save error: {}", e))?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn pangolin_api_request(
    app_handle: tauri::AppHandle,
    method: String,
    path: String,
    query_params: Option<HashMap<String, String>>,
    body: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    // 1. Get URL and Token
    let config_path = get_pangolin_config_path(&app_handle)?;
    let config = if config_path.exists() {
        let content = std::fs::read_to_string(config_path).map_err(|e| e.to_string())?;
        serde_json::from_str::<PangolinConfig>(&content).unwrap_or_else(|_| PangolinConfig {
            api_url: "https://api.pangolin.net".to_string(),
            org_id: None,
        })
    } else {
        PangolinConfig {
            api_url: "https://api.pangolin.net".to_string(),
            org_id: None,
        }
    };

    let keyring_service = "JarvisPangolin";
    let keyring_entry = keyring::Entry::new(keyring_service, "api_key")
        .map_err(|e| format!("Keyring init error: {}", e))?;
    let api_key = keyring_entry.get_password()
        .map_err(|_| "Pangolin API Key not configured. Please go to settings.".to_string())?;

    // 2. Build URL: combine config.api_url + path
    let base = config.api_url.trim_end_matches('/');
    let sub_path = path.trim_start_matches('/');
    let url_str = if sub_path.is_empty() {
        base.to_string()
    } else {
        format!("{}/{}", base, sub_path)
    };

    // 3. Perform request
    let client = reqwest::Client::new();
    let req_method = match method.to_uppercase().as_str() {
        "GET" => reqwest::Method::GET,
        "POST" => reqwest::Method::POST,
        "PUT" => reqwest::Method::PUT,
        "DELETE" => reqwest::Method::DELETE,
        "PATCH" => reqwest::Method::PATCH,
        _ => return Err(format!("Unsupported HTTP method: {}", method)),
    };

    let mut request = client.request(req_method, &url_str)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json");

    if let Some(ref q) = query_params {
        request = request.query(q);
    }

    if let Some(ref b) = body {
        request = request.json(b);
    }

    let response = request.send().await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    let status = response.status();
    let body_text = response.text().await
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    if !status.is_success() {
        if let Ok(err_json) = serde_json::from_str::<serde_json::Value>(&body_text) {
            if let Some(msg) = err_json.get("message").and_then(|m| m.as_str()) {
                return Err(msg.to_string());
            }
        }
        return Err(format!("HTTP Error {}: {}", status, body_text));
    }

    if body_text.is_empty() {
        return Ok(serde_json::json!({ "success": true, "message": "No content", "status": status.as_u16() }));
    }

    let parsed_json: serde_json::Value = serde_json::from_str(&body_text)
        .map_err(|e| format!("Failed to parse response JSON: {}\nResponse: {}", e, body_text))?;

    Ok(parsed_json)
}
