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
    pub db_password: Option<String>,
    // Off-site destination: "download" (default) | "s3" | "sftp"
    #[serde(default)]
    pub destination: Option<String>,
    #[serde(default)]
    pub dest_endpoint: Option<String>,
    #[serde(default)]
    pub dest_region: Option<String>,
    #[serde(default)]
    pub dest_bucket: Option<String>,
    #[serde(default)]
    pub dest_path: Option<String>,
    #[serde(default)]
    pub dest_host: Option<String>,
    #[serde(default)]
    pub dest_port: Option<String>,
    #[serde(default)]
    pub dest_user: Option<String>,
    // Secrets — never persisted to disk, only to the OS keyring.
    pub dest_access_key: Option<String>,
    pub dest_secret_key: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ResticRepo {
    pub id: String,
    pub name: String,
    pub repo_type: String, // "local" | "s3" | "sftp" | "b2" | "rest" | "rclone"
    pub path_or_url: String,
    #[serde(default)]
    pub s3_endpoint: Option<String>,
    #[serde(default)]
    pub s3_region: Option<String>,
    #[serde(default)]
    pub s3_bucket: Option<String>,
    #[serde(default)]
    pub env_vars: Option<HashMap<String, String>>,
    #[serde(default)]
    pub use_sudo: bool,
    pub password: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProfileExtras {
    #[serde(default)]
    pub runbooks: Vec<Runbook>,
    #[serde(default)]
    pub backup_templates: Vec<BackupTemplate>,
    #[serde(default)]
    pub restic_repos: Vec<ResticRepo>,
    #[serde(default)]
    pub alert_thresholds: AlertThresholds,
}

impl Default for ProfileExtras {
    fn default() -> Self {
        Self {
            runbooks: Vec::new(),
            backup_templates: Vec::new(),
            restic_repos: Vec::new(),
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
    
    // Clear secrets before serializing so they are never persisted to disk
    let mut data_to_save = data.clone();
    for extras in data_to_save.values_mut() {
        for tpl in &mut extras.backup_templates {
            tpl.db_password = None;
            tpl.dest_access_key = None;
            tpl.dest_secret_key = None;
        }
        for repo in &mut extras.restic_repos {
            repo.password = None;
            repo.access_key = None;
            repo.secret_key = None;
        }
    }

    let content = serde_json::to_string_pretty(&data_to_save)
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
    let dest_service = "JarvisBackupDest";
    for tpl in &mut extras.backup_templates {
        if let Ok(entry) = keyring::Entry::new(keyring_service, &tpl.id) {
            tpl.db_password = entry.get_password().ok();
        }
        if let Ok(entry) = keyring::Entry::new(dest_service, &format!("{}-access", tpl.id)) {
            tpl.dest_access_key = entry.get_password().ok();
        }
        if let Ok(entry) = keyring::Entry::new(dest_service, &format!("{}-secret", tpl.id)) {
            tpl.dest_secret_key = entry.get_password().ok();
        }
    }

    // Load restic_repos credentials
    let restic_service = "JarvisResticSecrets";
    for repo in &mut extras.restic_repos {
        if let Ok(entry) = keyring::Entry::new(restic_service, &format!("{}-password", repo.id)) {
            repo.password = entry.get_password().ok();
        }
        if let Ok(entry) = keyring::Entry::new(restic_service, &format!("{}-access", repo.id)) {
            repo.access_key = entry.get_password().ok();
        }
        if let Ok(entry) = keyring::Entry::new(restic_service, &format!("{}-secret", repo.id)) {
            repo.secret_key = entry.get_password().ok();
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

    // Save db_password and destination secrets to keyring for each template
    let keyring_service = "JarvisBackupDB";
    let dest_service = "JarvisBackupDest";
    for tpl in &extras.backup_templates {
        if let Some(ref pass) = tpl.db_password {
            if let Ok(entry) = keyring::Entry::new(keyring_service, &tpl.id) {
                entry.set_password(pass).ok();
            }
        }
        if let Some(ref ak) = tpl.dest_access_key {
            if let Ok(entry) = keyring::Entry::new(dest_service, &format!("{}-access", tpl.id)) {
                entry.set_password(ak).ok();
            }
        }
        if let Some(ref sk) = tpl.dest_secret_key {
            if let Ok(entry) = keyring::Entry::new(dest_service, &format!("{}-secret", tpl.id)) {
                entry.set_password(sk).ok();
            }
        }
    }

    // Save restic secrets to keyring
    let restic_service = "JarvisResticSecrets";
    for repo in &extras.restic_repos {
        if let Some(ref pass) = repo.password {
            if let Ok(entry) = keyring::Entry::new(restic_service, &format!("{}-password", repo.id)) {
                entry.set_password(pass).ok();
            }
        }
        if let Some(ref ak) = repo.access_key {
            if let Ok(entry) = keyring::Entry::new(restic_service, &format!("{}-access", repo.id)) {
                entry.set_password(ak).ok();
            }
        }
        if let Some(ref sk) = repo.secret_key {
            if let Ok(entry) = keyring::Entry::new(restic_service, &format!("{}-secret", repo.id)) {
                entry.set_password(sk).ok();
            }
        }
    }

    all.insert(profile_id, extras);
    save_all(&app_handle, &all)
}
