use crate::app_error::AppError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AlertThresholds {
    pub enabled: bool,
    pub disk_pct: u8,
    pub ram_pct: u8,
    pub cpu_pct: u8,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            enabled: true,
            disk_pct: 85,
            ram_pct: 90,
            cpu_pct: 95,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Runbook {
    pub id: String,
    pub name: String,
    pub command: String,
    pub use_sudo: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BackupTemplate {
    pub id: String,
    pub name: String,
    pub backup_type: String,
    pub source_path: String,
    pub docker_container: Option<String>,
    pub db_name: Option<String>,
    pub db_user: Option<String>,
    #[serde(skip)]
    pub db_password: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProfileExtras {
    #[serde(default)]
    pub runbooks: Vec<Runbook>,
    #[serde(default)]
    pub backup_templates: Vec<BackupTemplate>,
    #[serde(default)]
    pub alert_thresholds: AlertThresholds,
}

impl Default for ProfileExtras {
    fn default() -> Self {
        Self {
            runbooks: Vec::new(),
            backup_templates: Vec::new(),
            alert_thresholds: AlertThresholds::default(),
        }
    }
}

fn extras_path(app_handle: &AppHandle) -> Result<PathBuf, AppError> {
    let mut path = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| AppError::with_details("APP_CONFIG_DIR_FAILED", e.to_string()))?;
    fs::create_dir_all(&path).map_err(|e| {
        AppError::with_details("PROFILE_EXTRAS_CONFIG_DIR_FAILED", e.to_string())
    })?;
    path.push("profile_extras.json");
    Ok(path)
}

fn load_all(app_handle: &AppHandle) -> Result<HashMap<String, ProfileExtras>, AppError> {
    let path = extras_path(app_handle)?;
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let content = fs::read_to_string(path)
        .map_err(|e| AppError::with_details("PROFILE_EXTRAS_READ_FAILED", e.to_string()))?;
    Ok(serde_json::from_str(&content).unwrap_or_default())
}

fn save_all(
    app_handle: &AppHandle,
    data: &HashMap<String, ProfileExtras>,
) -> Result<(), AppError> {
    let path = extras_path(app_handle)?;
    let content = serde_json::to_string_pretty(data)
        .map_err(|e| AppError::with_details("JSON_SERIALIZE_FAILED", e.to_string()))?;
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, &content)
        .map_err(|e| AppError::with_details("PROFILE_EXTRAS_WRITE_FAILED", e.to_string()))?;
    fs::rename(&tmp_path, &path)
        .map_err(|e| AppError::with_details("PROFILE_EXTRAS_RENAME_FAILED", e.to_string()))
}

#[tauri::command]
pub fn get_profile_extras(
    app_handle: AppHandle,
    profile_id: String,
) -> Result<ProfileExtras, AppError> {
    let all = load_all(&app_handle)?;
    let mut extras = all.get(&profile_id).cloned().unwrap_or_default();

    // Load db_password from keyring for each backup template
    let keyring_service = "JarvisBackupDB";
    for tpl in &mut extras.backup_templates {
        if let Ok(entry) = keyring::Entry::new(keyring_service, &tpl.id) {
            tpl.db_password = entry.get_password().ok();
        }
    }

    Ok(extras)
}

#[tauri::command]
pub fn save_profile_extras(
    app_handle: AppHandle,
    profile_id: String,
    extras: ProfileExtras,
) -> Result<(), AppError> {
    let mut all = load_all(&app_handle)?;

    // Save db_password to keyring for each backup template
    let keyring_service = "JarvisBackupDB";
    for tpl in &extras.backup_templates {
        if let Some(ref pass) = tpl.db_password {
            if let Ok(entry) = keyring::Entry::new(keyring_service, &tpl.id) {
                entry.set_password(pass).ok();
            }
        }
    }

    all.insert(profile_id, extras);
    save_all(&app_handle, &all)
}
