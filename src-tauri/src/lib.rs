mod app_error;
mod ssh;
mod sftp_transfer;
mod du_size;
mod sftp_find;
mod pangolin;
mod profile_extras;

use app_error::AppError;
use parking_lot::Mutex;
use std::sync::Arc;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::{Manager, Emitter, State, AppHandle};
use serde::{Serialize, Deserialize};
use keyring::Entry;
use ssh::{SshConnection, ServerStats, ExtendedServerStats, FileInfo};
use sftp_transfer::{
    build_delete_jobs, build_move_jobs, build_upload_jobs, collect_local_files,
    collect_remote_files, get_downloads_dir, DeleteItem, MoveItem, TransferRunner,
    UploadItem,
};
use std::sync::atomic::AtomicBool;
use tokio::sync::mpsc;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Profile {
    pub id: String,
    pub label: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: String,
    pub key_path: Option<String>,
}

#[derive(Clone)]
pub struct SshCreds {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub private_key: Option<String>,
    pub passphrase: Option<String>,
}

pub struct AppState {
    pub connection: Arc<Mutex<Option<SshConnection>>>,
    pub sudo_password: Arc<Mutex<Option<String>>>,
    pub ssh_creds: Arc<Mutex<Option<SshCreds>>>,
    pub terminal_txs: Arc<Mutex<HashMap<String, mpsc::Sender<String>>>>,
    pub terminal_cancels: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>>,
    pub docker_log_cancel: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    pub docker_compose_cancel: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    pub sftp_transfer_cancel: Arc<AtomicBool>,
    pub sftp_transfer_running: Arc<AtomicBool>,
}

fn stop_terminal_sessions(state: &AppState) {
    let txs: Vec<_> = state.terminal_txs.lock().drain().collect();
    drop(txs);
    let cancels: Vec<(String, _)> = state.terminal_cancels.lock().drain().collect();
    for (_, cancel) in cancels {
        cancel.send(()).ok();
    }
}

fn stop_terminal_session(state: &AppState, session_id: &str) {
    state.terminal_txs.lock().remove(session_id);
    if let Some(cancel) = state.terminal_cancels.lock().remove(session_id) {
        cancel.send(()).ok();
    }
}

fn no_ssh_connection() -> AppError {
    AppError::new("NO_SSH_CONNECTION")
}

fn get_profiles_path(app_handle: &AppHandle) -> Result<PathBuf, AppError> {
    let mut path = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| AppError::with_details("APP_CONFIG_DIR_FAILED", e.to_string()))?;
    fs::create_dir_all(&path).map_err(|e| {
        AppError::with_details("CONFIG_DIR_CREATE_FAILED", e.to_string())
    })?;
    path.push("profiles.json");
    Ok(path)
}

#[tauri::command]
fn get_profiles(app_handle: AppHandle) -> Result<Vec<Profile>, AppError> {
    let path = get_profiles_path(&app_handle)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path)
        .map_err(|e| AppError::with_details("PROFILES_READ_FAILED", e.to_string()))?;
    let profiles: Vec<Profile> = serde_json::from_str(&content).unwrap_or_else(|_| Vec::new());
    Ok(profiles)
}

#[tauri::command]
fn save_profile(
    app_handle: AppHandle,
    profile: Profile,
    password: Option<String>,
    key_passphrase: Option<String>,
) -> Result<(), AppError> {
    let keyring_service = "JarvisServerManager";

    if let Some(pass) = password {
        let entry = Entry::new(keyring_service, &format!("{}_pass", profile.id))
            .map_err(|e| AppError::with_details("KEYRING_INIT_FAILED", e.to_string()))?;
        entry
            .set_password(&pass)
            .map_err(|e| AppError::with_details("KEYRING_PASSWORD_SAVE_FAILED", e.to_string()))?;
    }

    if let Some(passphrase) = key_passphrase {
        let entry = Entry::new(keyring_service, &format!("{}_passphrase", profile.id))
            .map_err(|e| AppError::with_details("KEYRING_INIT_FAILED", e.to_string()))?;
        entry
            .set_password(&passphrase)
            .map_err(|e| AppError::with_details("KEYRING_PASSPHRASE_SAVE_FAILED", e.to_string()))?;
    }

    let path = get_profiles_path(&app_handle)?;
    let mut profiles = get_profiles(app_handle.clone())?;

    if let Some(pos) = profiles.iter().position(|p| p.id == profile.id) {
        profiles[pos] = profile;
    } else {
        profiles.push(profile);
    }

    let content = serde_json::to_string_pretty(&profiles)
        .map_err(|e| AppError::with_details("JSON_SERIALIZE_FAILED", e.to_string()))?;
    fs::write(path, content)
        .map_err(|e| AppError::with_details("PROFILES_WRITE_FAILED", e.to_string()))?;

    Ok(())
}

#[tauri::command]
fn delete_profile(app_handle: AppHandle, id: String) -> Result<(), AppError> {
    let keyring_service = "JarvisServerManager";

    if let Ok(entry) = Entry::new(keyring_service, &format!("{}_pass", id)) {
        entry.delete_password().ok();
    }
    if let Ok(entry) = Entry::new(keyring_service, &format!("{}_passphrase", id)) {
        entry.delete_password().ok();
    }

    let path = get_profiles_path(&app_handle)?;
    let mut profiles = get_profiles(app_handle.clone())?;
    profiles.retain(|p| p.id != id);

    let content = serde_json::to_string_pretty(&profiles)
        .map_err(|e| AppError::with_details("JSON_SERIALIZE_FAILED", e.to_string()))?;
    fs::write(path, content)
        .map_err(|e| AppError::with_details("PROFILES_WRITE_FAILED", e.to_string()))?;

    Ok(())
}

#[tauri::command]
async fn connect_ssh(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    profile_id: String,
) -> Result<ServerStats, AppError> {
    disconnect_ssh(state.clone()).await.ok();

    let profiles = get_profiles(app_handle.clone())?;
    let profile = profiles
        .iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| AppError::new("PROFILE_NOT_FOUND"))?;

    let keyring_service = "JarvisServerManager";
    let password = if profile.auth_type == "password" {
        let entry = Entry::new(keyring_service, &format!("{}_pass", profile.id)).ok();
        entry.and_then(|e| e.get_password().ok())
    } else {
        None
    };

    let passphrase = if profile.auth_type == "key" {
        let entry = Entry::new(keyring_service, &format!("{}_passphrase", profile.id)).ok();
        entry.and_then(|e| e.get_password().ok())
    } else {
        None
    };

    let private_key_path = if profile.auth_type == "key" {
        profile.key_path.as_ref().map(std::path::Path::new)
    } else {
        None
    };

    let conn = SshConnection::connect(
        &profile.host,
        profile.port,
        &profile.username,
        password.as_deref(),
        private_key_path,
        passphrase.as_deref(),
    )
    .await?;

    let stats = conn.get_stats().await?;

    let creds = SshCreds {
        host: profile.host.clone(),
        port: profile.port,
        username: profile.username.clone(),
        password,
        private_key: profile.key_path.clone(),
        passphrase,
    };

    *state.ssh_creds.lock() = Some(creds);
    *state.connection.lock() = Some(conn);
    *state.sudo_password.lock() = None;

    Ok(stats)
}

#[tauri::command]
async fn disconnect_ssh(state: State<'_, AppState>) -> Result<(), AppError> {
    *state.connection.lock() = None;
    *state.ssh_creds.lock() = None;
    *state.sudo_password.lock() = None;
    stop_terminal_sessions(&state);
    Ok(())
}

#[tauri::command]
async fn get_server_stats(state: State<'_, AppState>) -> Result<ServerStats, AppError> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or_else(no_ssh_connection)?.clone()
    };
    conn.get_stats().await
}

#[tauri::command]
async fn get_extended_server_stats(
    state: State<'_, AppState>,
) -> Result<ExtendedServerStats, AppError> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or_else(no_ssh_connection)?.clone()
    };
    conn.get_extended_stats().await
}

#[tauri::command]
async fn switch_ssh(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    profile_id: String,
) -> Result<ServerStats, AppError> {
    connect_ssh(state, app_handle, profile_id).await
}

#[tauri::command]
async fn exec_custom_command(
    state: State<'_, AppState>,
    cmd: String,
    use_sudo: bool,
) -> Result<String, AppError> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or_else(no_ssh_connection)?.clone()
    };

    if use_sudo {
        let sudo_pass = {
            let sudo_pass_guard = state.sudo_password.lock();
            sudo_pass_guard
                .as_ref()
                .ok_or_else(|| AppError::new("SUDO_PASSWORD_REQUIRED"))?
                .clone()
        };

        let formatted_cmd = format!("echo '{}' | sudo -S -- {}", sudo_pass, cmd);
        let (exit_code, stdout, stderr) = conn.exec(&formatted_cmd).await?;

        if exit_code != 0 {
            if stderr.contains("incorrect password attempt") || stderr.contains("złe hasło") {
                return Err(AppError::new("SUDO_PASSWORD_INCORRECT"));
            }
            return Err(AppError::with_details(
                "REMOTE_COMMAND_FAILED",
                format!("exit={}\nstderr={}\nstdout={}", exit_code, stderr, stdout),
            ));
        }
        Ok(stdout)
    } else {
        let (exit_code, stdout, stderr) = conn.exec(&cmd).await?;
        if exit_code != 0 {
            return Err(AppError::with_details(
                "REMOTE_COMMAND_FAILED",
                format!("exit={}\nstderr={}\nstdout={}", exit_code, stderr, stdout),
            ));
        }
        Ok(stdout)
    }
}

#[tauri::command]
async fn set_sudo_password(state: State<'_, AppState>, password: String) -> Result<(), AppError> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or_else(no_ssh_connection)?.clone()
    };

    let test_cmd = format!("echo '{}' | sudo -S id", password);
    let (exit_code, _stdout, stderr) = conn.exec(&test_cmd).await?;

    if exit_code != 0 {
        if stderr.contains("incorrect password attempt") || stderr.contains("złe hasło") {
            return Err(AppError::new("SUDO_PASSWORD_INCORRECT"));
        }
        return Err(AppError::with_details("SUDO_VERIFICATION_FAILED", stderr));
    }

    *state.sudo_password.lock() = Some(password);
    Ok(())
}

#[tauri::command]
fn has_sudo_password(state: State<'_, AppState>) -> bool {
    state.sudo_password.lock().is_some()
}

#[tauri::command]
async fn sftp_list(state: State<'_, AppState>, path: String) -> Result<Vec<FileInfo>, AppError> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or_else(no_ssh_connection)?.clone()
    };
    conn.sftp_list_dir(&path).await
}

#[tauri::command]
async fn sftp_dir_size(state: State<'_, AppState>, path: String) -> Result<u64, AppError> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or_else(no_ssh_connection)?.clone()
    };
    conn.sftp_dir_size(&path).await
}

#[tauri::command]
async fn sftp_find(
    state: State<'_, AppState>,
    root: String,
    query: String,
    hide_hidden: bool,
) -> Result<Vec<FileInfo>, AppError> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or_else(no_ssh_connection)?.clone()
    };
    conn.sftp_find(&root, &query, hide_hidden).await
}

#[tauri::command]
async fn sftp_read(state: State<'_, AppState>, path: String) -> Result<String, AppError> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or_else(no_ssh_connection)?.clone()
    };
    conn.sftp_read_file(&path).await
}

#[tauri::command]
async fn sftp_write(
    state: State<'_, AppState>,
    path: String,
    content: String,
) -> Result<(), AppError> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or_else(no_ssh_connection)?.clone()
    };
    conn.sftp_write_file(&path, &content).await
}

#[tauri::command]
async fn sftp_create_dir(state: State<'_, AppState>, path: String) -> Result<(), AppError> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or_else(no_ssh_connection)?.clone()
    };
    conn.sftp_create_dir(&path).await
}

#[tauri::command]
async fn sftp_delete(
    state: State<'_, AppState>,
    path: String,
    is_dir: bool,
) -> Result<(), AppError> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or_else(no_ssh_connection)?.clone()
    };
    conn.sftp_delete_file(&path, is_dir).await
}

#[tauri::command]
async fn sftp_rename(
    state: State<'_, AppState>,
    src: String,
    dest: String,
) -> Result<(), AppError> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or_else(no_ssh_connection)?.clone()
    };
    conn.sftp_rename(&src, &dest).await
}

fn validate_shell(shell: &str) -> Result<(), AppError> {
    if shell.is_empty() || shell.len() > 64 {
        return Err(AppError::new("INVALID_SHELL"));
    }
    if !shell.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/')) {
        return Err(AppError::new("INVALID_SHELL"));
    }
    Ok(())
}

fn validate_container_id(id: &str) -> Result<(), AppError> {
    if id.is_empty() || id.len() > 128 {
        return Err(AppError::new("INVALID_CONTAINER_ID"));
    }
    if !id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/')) {
        return Err(AppError::new("INVALID_CONTAINER_ID"));
    }
    Ok(())
}

#[tauri::command]
async fn start_terminal(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    session_id: String,
    container_id: Option<String>,
    use_sudo: Option<bool>,
    shell: Option<String>,
) -> Result<(), AppError> {
    let use_sudo = use_sudo.unwrap_or(false);
    let shell = shell.unwrap_or_else(|| "/bin/sh".to_string());
    let sudo_pass = if container_id.is_some() && use_sudo {
        let sudo_pass_guard = state.sudo_password.lock();
        Some(
            sudo_pass_guard
                .as_ref()
                .ok_or_else(|| AppError::new("SUDO_PASSWORD_REQUIRED"))?
                .clone(),
        )
    } else {
        None
    };

    if let Some(ref id) = container_id {
        validate_container_id(id)?;
        validate_shell(&shell)?;
    }

    stop_terminal_session(&state, &session_id);

    let creds_guard = state.ssh_creds.lock();
    let creds = creds_guard.as_ref().ok_or_else(no_ssh_connection)?.clone();

    let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel::<()>();
    state
        .terminal_cancels
        .lock()
        .insert(session_id.clone(), cancel_tx);

    let (tx, mut rx) = mpsc::channel::<String>(100);
    state.terminal_txs.lock().insert(session_id.clone(), tx);

    let app_handle_clone = app_handle.clone();
    let session_id_clone = session_id.clone();

    tokio::spawn(async move {
        let run_terminal = async {
            let private_key_path = creds.private_key.as_ref().map(std::path::Path::new);
            let conn = SshConnection::connect(
                &creds.host,
                creds.port,
                &creds.username,
                creds.password.as_deref(),
                private_key_path,
                creds.passphrase.as_deref(),
            )
            .await?;

            let mut channel = {
                let session = conn.session.lock().await;
                let channel = session
                    .channel_open_session()
                    .await
                    .map_err(|e| AppError::with_details("SSH_CHANNEL_OPEN_FAILED", e.to_string()))?;

                channel
                    .request_pty(true, "xterm-256color", 80, 24, 0, 0, &[])
                    .await
                    .map_err(|e| AppError::with_details("SSH_PTY_FAILED", e.to_string()))?;

                if let Some(ref id) = container_id {
                    let docker_cmd = format!(
                        "docker exec -it {} env TERM=xterm-256color {} -i",
                        id, shell
                    );
                    let cmd = if use_sudo {
                        if let Some(ref pass) = sudo_pass {
                            format!("echo '{}' | sudo -S -- {}", pass, docker_cmd)
                        } else {
                            docker_cmd
                        }
                    } else {
                        docker_cmd
                    };

                    channel
                        .exec(true, cmd.as_bytes())
                        .await
                        .map_err(|e| {
                            AppError::with_details("CONTAINER_SHELL_START_FAILED", e.to_string())
                        })?;
                } else {
                    channel
                        .request_shell(true)
                        .await
                        .map_err(|e| AppError::with_details("SSH_SHELL_FAILED", e.to_string()))?;
                }

                channel
            };

            let app_handle_read = app_handle_clone.clone();

            loop {
                tokio::select! {
                    msg = channel.wait() => {
                        match msg {
                            Some(russh::ChannelMsg::Data { data }) => {
                                let stdout_str = String::from_utf8_lossy(&data).to_string();
                                app_handle_read.emit(&format!("terminal-stdout-{}", session_id_clone), stdout_str).ok();
                            }
                            Some(russh::ChannelMsg::ExtendedData { data, ext }) => {
                                if ext == 1 {
                                    let stderr_str = String::from_utf8_lossy(&data).to_string();
                                    app_handle_read.emit(&format!("terminal-stdout-{}", session_id_clone), stderr_str).ok();
                                }
                            }
                            Some(russh::ChannelMsg::ExitStatus { exit_status }) => {
                                app_handle_read.emit(
                                    &format!("terminal-stdout-{}", session_id_clone),
                                    format!("\r\n[Połączenie terminala zamknięte, kod: {}]\r\n", exit_status),
                                ).ok();
                                break;
                            }
                            Some(russh::ChannelMsg::Eof) => {
                                app_handle_read.emit(
                                    &format!("terminal-stdout-{}", session_id_clone),
                                    "\r\n[Połączenie terminala zamknięte]\r\n".to_string(),
                                ).ok();
                                break;
                            }
                            None => break,
                            _ => {}
                        }
                    }
                    input_opt = rx.recv() => {
                        match input_opt {
                            Some(input_data) => {
                                if let Err(e) = channel.data(input_data.as_bytes()).await {
                                    app_handle_read.emit(
                                        &format!("terminal-stdout-{}", session_id_clone),
                                        format!("\r\n[Błąd zapisu do terminala: {}]\r\n", e),
                                    ).ok();
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                    _ = &mut cancel_rx => {
                        break;
                    }
                }
            }

            Ok::<(), AppError>(())
        };

        if let Err(e) = run_terminal.await {
            app_handle_clone
                .emit(
                    &format!("terminal-stdout-{}", session_id_clone),
                    format!("\r\n[Błąd sesji SSH terminala: {}]\r\n", e),
                )
                .ok();
        }
    });

    Ok(())
}

#[tauri::command]
async fn stop_terminal(state: State<'_, AppState>, session_id: String) -> Result<(), AppError> {
    stop_terminal_session(&state, &session_id);
    Ok(())
}

#[tauri::command]
fn send_terminal_input(
    state: State<'_, AppState>,
    session_id: String,
    input: String,
) -> Result<(), AppError> {
    let txs_guard = state.terminal_txs.lock();
    if let Some(tx) = txs_guard.get(&session_id) {
        tx.try_send(input).map_err(|e| {
            AppError::with_details("TERMINAL_SEND_FAILED", e.to_string())
        })?;
        Ok(())
    } else {
        Err(AppError::new("TERMINAL_NOT_RUNNING"))
    }
}

#[tauri::command]
fn open_external_terminal(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    profile_id: String,
    container_id: Option<String>,
    use_sudo: Option<bool>,
    shell: Option<String>,
) -> Result<(), AppError> {
    let profiles = get_profiles(app_handle)?;
    let profile = profiles
        .iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| AppError::new("PROFILE_NOT_FOUND"))?;

    let use_sudo = use_sudo.unwrap_or(false);
    let shell = shell.unwrap_or_else(|| "/bin/sh".to_string());

    let remote_cmd = if let Some(ref id) = container_id {
        validate_container_id(id)?;
        validate_shell(&shell)?;

        let docker_cmd = format!(
            "docker exec -it {} env TERM=xterm-256color {} -i",
            id, shell
        );

        if use_sudo {
            let sudo_pass = state.sudo_password.lock();
            let pass = sudo_pass
                .as_ref()
                .ok_or_else(|| AppError::new("SUDO_PASSWORD_REQUIRED"))?;
            format!("echo '{}' | sudo -S -- {}", pass, docker_cmd)
        } else {
            docker_cmd
        }
    } else {
        String::new()
    };

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;

        let ssh_args = if profile.auth_type == "key" {
            if let Some(ref key_path) = profile.key_path {
                format!(
                    "ssh -i \"{}\" -p {} {}@{}",
                    key_path, profile.port, profile.username, profile.host
                )
            } else {
                format!("ssh -p {} {}@{}", profile.port, profile.username, profile.host)
            }
        } else {
            format!("ssh -p {} {}@{}", profile.port, profile.username, profile.host)
        };

        let ssh_args = if !remote_cmd.is_empty() {
            format!("{} -t \"{}\"", ssh_args, remote_cmd)
        } else {
            ssh_args
        };

        let wt_res = Command::new("wt")
            .arg("cmd.exe")
            .arg("/k")
            .arg(&ssh_args)
            .spawn();

        if wt_res.is_err() {
            Command::new("cmd.exe")
                .arg("/c")
                .arg("start")
                .arg("cmd.exe")
                .arg("/k")
                .arg(&ssh_args)
                .spawn()
                .map_err(|e| AppError::with_details("EXTERNAL_TERMINAL_OPEN_FAILED", e.to_string()))?;
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (profile, remote_cmd);
        return Err(AppError::new("EXTERNAL_TERMINAL_WINDOWS_ONLY"));
    }

    Ok(())
}

fn spawn_transfer_batch(
    state: &AppState,
    app_handle: AppHandle,
    jobs: Vec<sftp_transfer::InternalJob>,
) -> Result<(), AppError> {
    if state
        .sftp_transfer_running
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        return Err(AppError::new("TRANSFER_ALREADY_RUNNING"));
    }

    state
        .sftp_transfer_cancel
        .store(false, std::sync::atomic::Ordering::Relaxed);
    state
        .sftp_transfer_running
        .store(true, std::sync::atomic::Ordering::Relaxed);

    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or_else(no_ssh_connection)?.clone()
    };

    let cancel = state.sftp_transfer_cancel.clone();
    let running = state.sftp_transfer_running.clone();
    let runner = TransferRunner::new(cancel);

    tokio::spawn(async move {
        runner.run_batch(conn, app_handle, jobs).await;
        running.store(false, std::sync::atomic::Ordering::Relaxed);
    });

    Ok(())
}

#[tauri::command]
async fn sftp_start_upload_batch(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    remote_dir: String,
    local_paths: Vec<String>,
) -> Result<u32, AppError> {
    let jobs = if local_paths.iter().any(|p| p.contains("::")) {
        let items: Vec<UploadItem> = local_paths
            .iter()
            .map(|pair| {
                let parts: Vec<&str> = pair.splitn(2, "::").collect();
                UploadItem {
                    local_path: parts[0].to_string(),
                    remote_path: parts.get(1).copied().unwrap_or("").to_string(),
                }
            })
            .collect();
        build_upload_jobs(items)
    } else {
        collect_local_files(&local_paths, &remote_dir)?
    };
    let count = jobs.len() as u32;
    spawn_transfer_batch(&state, app_handle, jobs)?;
    Ok(count)
}

#[tauri::command]
async fn sftp_start_download_batch(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    remote_paths: Vec<String>,
    local_dir: Option<String>,
) -> Result<u32, AppError> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or_else(no_ssh_connection)?.clone()
    };

    let base_dir = if let Some(dir) = local_dir {
        PathBuf::from(dir)
    } else {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        get_downloads_dir().join(format!("Jarvis-SFTP-{}", ts))
    };
    fs::create_dir_all(&base_dir).map_err(|e| {
        AppError::with_details("DOWNLOAD_DIR_CREATE_FAILED", e.to_string())
    })?;

    let sftp = sftp_transfer::open_sftp(&conn).await?;
    let mut jobs = Vec::new();
    for remote_path in &remote_paths {
        let name = remote_path
            .split('/')
            .filter(|s| !s.is_empty())
            .last()
            .unwrap_or("download");
        let local_path = base_dir.join(name);
        let mut path_jobs =
            collect_remote_files(&sftp, remote_path, &local_path, remote_path).await?;
        jobs.append(&mut path_jobs);
    }

    let count = jobs.len() as u32;
    spawn_transfer_batch(&state, app_handle, jobs)?;
    Ok(count)
}

#[tauri::command]
async fn sftp_start_move_batch(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    moves: Vec<MoveItem>,
) -> Result<u32, AppError> {
    let jobs = build_move_jobs(moves);
    let count = jobs.len() as u32;
    spawn_transfer_batch(&state, app_handle, jobs)?;
    Ok(count)
}

#[tauri::command]
async fn sftp_start_delete_batch(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    paths: Vec<DeleteItem>,
) -> Result<u32, AppError> {
    let jobs = build_delete_jobs(paths);
    let count = jobs.len() as u32;
    spawn_transfer_batch(&state, app_handle, jobs)?;
    Ok(count)
}

#[tauri::command]
async fn sftp_cancel_transfer(state: State<'_, AppState>) -> Result<(), AppError> {
    state
        .sftp_transfer_cancel
        .store(true, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn sftp_get_downloads_dir() -> String {
    get_downloads_dir().to_string_lossy().to_string()
}

fn pick_dialog_title(locale: &str, en: &str, pl: &str) -> String {
    if locale.starts_with("pl") {
        pl.to_string()
    } else {
        en.to_string()
    }
}

#[tauri::command]
fn sftp_pick_files(locale: String) -> Result<Vec<String>, AppError> {
    let title = pick_dialog_title(
        &locale,
        "Select files to upload",
        "Wybierz pliki do wysłania",
    );
    let files = rfd::FileDialog::new().set_title(&title).pick_files();
    Ok(files
        .unwrap_or_default()
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

#[tauri::command]
fn sftp_pick_folder(locale: String) -> Result<Option<String>, AppError> {
    let title = pick_dialog_title(
        &locale,
        "Select folder to upload",
        "Wybierz folder do wysłania",
    );
    Ok(rfd::FileDialog::new()
        .set_title(&title)
        .pick_folder()
        .map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
async fn sftp_get_home_dir(state: State<'_, AppState>) -> Result<String, AppError> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or_else(no_ssh_connection)?.clone()
    };
    let sftp = conn.get_sftp().await?;
    let path = sftp
        .canonicalize(".")
        .await
        .map_err(|e| AppError::with_details("HOME_DIR_READ_FAILED", e.to_string()))?;
    Ok(path)
}

#[tauri::command]
async fn start_container_logs(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    container_id: String,
    tail: Option<u32>,
    use_sudo: bool,
) -> Result<(), AppError> {
    {
        let mut cancel_guard = state.docker_log_cancel.lock();
        if let Some(tx) = cancel_guard.take() {
            tx.send(()).ok();
        }
    }

    let creds_guard = state.ssh_creds.lock();
    let creds = creds_guard.as_ref().ok_or_else(no_ssh_connection)?.clone();

    let sudo_pass = if use_sudo {
        let sudo_pass_guard = state.sudo_password.lock();
        sudo_pass_guard.clone()
    } else {
        None
    };

    let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel::<()>();
    *state.docker_log_cancel.lock() = Some(cancel_tx);

    let tail_num = tail.unwrap_or(200);
    let app = app_handle.clone();

    tokio::spawn(async move {
        let run_logs = async {
            let private_key_path = creds.private_key.as_ref().map(std::path::Path::new);
            let conn = SshConnection::connect(
                &creds.host,
                creds.port,
                &creds.username,
                creds.password.as_deref(),
                private_key_path,
                creds.passphrase.as_deref(),
            )
            .await?;

            let session = conn.session.lock().await;
            let mut channel = session
                .channel_open_session()
                .await
                .map_err(|e| AppError::with_details("SSH_CHANNEL_OPEN_FAILED", e.to_string()))?;

            let cmd = if use_sudo {
                if let Some(ref pass) = sudo_pass {
                    format!(
                        "echo '{}' | sudo -S docker logs -f --tail {} {}",
                        pass, tail_num, container_id
                    )
                } else {
                    format!("docker logs -f --tail {} {}", tail_num, container_id)
                }
            } else {
                format!("docker logs -f --tail {} {}", tail_num, container_id)
            };

            channel
                .exec(true, cmd.as_bytes())
                .await
                .map_err(|e| AppError::with_details("DOCKER_LOGS_START_FAILED", e.to_string()))?;

            loop {
                tokio::select! {
                    msg = channel.wait() => {
                        match msg {
                            Some(russh::ChannelMsg::Data { data }) => {
                                let text = String::from_utf8_lossy(&data).to_string();
                                app.emit("docker-log-data", text).ok();
                            }
                            Some(russh::ChannelMsg::ExtendedData { data, ext }) => {
                                if ext == 1 {
                                    let text = String::from_utf8_lossy(&data).to_string();
                                    app.emit("docker-log-data", text).ok();
                                }
                            }
                            Some(russh::ChannelMsg::Eof) | None => {
                                break;
                            }
                            _ => {}
                        }
                    }
                    _ = &mut cancel_rx => {
                        break;
                    }
                }
            }

            Ok::<(), AppError>(())
        };

        if let Err(e) = run_logs.await {
            app.emit(
                "docker-log-data",
                format!("\n[Log stream error: {}]\n", e),
            )
            .ok();
        }
    });

    Ok(())
}

#[tauri::command]
async fn stop_container_logs(state: State<'_, AppState>) -> Result<(), AppError> {
    let mut cancel_guard = state.docker_log_cancel.lock();
    if let Some(tx) = cancel_guard.take() {
        tx.send(()).ok();
    }
    Ok(())
}

#[tauri::command]
async fn start_compose_pull(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    config_file: String,
    use_sudo: bool,
) -> Result<(), AppError> {
    {
        let mut cancel_guard = state.docker_compose_cancel.lock();
        if let Some(tx) = cancel_guard.take() {
            tx.send(()).ok();
        }
    }

    let creds_guard = state.ssh_creds.lock();
    let creds = creds_guard.as_ref().ok_or_else(no_ssh_connection)?.clone();

    let sudo_pass = if use_sudo {
        let sudo_pass_guard = state.sudo_password.lock();
        sudo_pass_guard.clone()
    } else {
        None
    };

    let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel::<()>();
    *state.docker_compose_cancel.lock() = Some(cancel_tx);

    let app = app_handle.clone();

    tokio::spawn(async move {
        let run_pull = async {
            let private_key_path = creds.private_key.as_ref().map(std::path::Path::new);
            let conn = SshConnection::connect(
                &creds.host,
                creds.port,
                &creds.username,
                creds.password.as_deref(),
                private_key_path,
                creds.passphrase.as_deref(),
            )
            .await?;

            let session = conn.session.lock().await;
            let mut channel = session
                .channel_open_session()
                .await
                .map_err(|e| AppError::with_details("SSH_CHANNEL_OPEN_FAILED", e.to_string()))?;

            let cmd = if use_sudo {
                if let Some(ref pass) = sudo_pass {
                    format!(
                        "echo '{}' | sudo -S docker compose -f {} pull",
                        pass, config_file
                    )
                } else {
                    format!("docker compose -f {} pull", config_file)
                }
            } else {
                format!("docker compose -f {} pull", config_file)
            };

            channel
                .exec(true, cmd.as_bytes())
                .await
                .map_err(|e| AppError::with_details("COMPOSE_PULL_START_FAILED", e.to_string()))?;

            loop {
                tokio::select! {
                    msg = channel.wait() => {
                        match msg {
                            Some(russh::ChannelMsg::Data { data }) => {
                                let text = String::from_utf8_lossy(&data).to_string();
                                app.emit("compose-pull-data", text).ok();
                            }
                            Some(russh::ChannelMsg::ExtendedData { data, ext }) => {
                                if ext == 1 {
                                    let text = String::from_utf8_lossy(&data).to_string();
                                    app.emit("compose-pull-data", text).ok();
                                }
                            }
                            Some(russh::ChannelMsg::Eof) | None => {
                                break;
                            }
                            _ => {}
                        }
                    }
                    _ = &mut cancel_rx => {
                        break;
                    }
                }
            }

            Ok::<(), AppError>(())
        };

        if let Err(e) = run_pull.await {
            app.emit(
                "compose-pull-data",
                format!("\n[Pull error: {}]\n", e),
            )
            .ok();
        }
    });

    Ok(())
}

#[tauri::command]
async fn stop_compose_pull(state: State<'_, AppState>) -> Result<(), AppError> {
    let mut cancel_guard = state.docker_compose_cancel.lock();
    if let Some(tx) = cancel_guard.take() {
        tx.send(()).ok();
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .manage(AppState {
            connection: Arc::new(Mutex::new(None)),
            sudo_password: Arc::new(Mutex::new(None)),
            ssh_creds: Arc::new(Mutex::new(None)),
            terminal_txs: Arc::new(Mutex::new(HashMap::new())),
            terminal_cancels: Arc::new(Mutex::new(HashMap::new())),
            docker_log_cancel: Arc::new(Mutex::new(None)),
            docker_compose_cancel: Arc::new(Mutex::new(None)),
            sftp_transfer_cancel: Arc::new(AtomicBool::new(false)),
            sftp_transfer_running: Arc::new(AtomicBool::new(false)),
        })
        .invoke_handler(tauri::generate_handler![
            get_profiles,
            save_profile,
            delete_profile,
            connect_ssh,
            disconnect_ssh,
            switch_ssh,
            get_server_stats,
            get_extended_server_stats,
            exec_custom_command,
            set_sudo_password,
            has_sudo_password,
            sftp_list,
            sftp_dir_size,
            sftp_find,
            sftp_read,
            sftp_write,
            sftp_create_dir,
            sftp_delete,
            sftp_rename,
            start_terminal,
            stop_terminal,
            send_terminal_input,
            open_external_terminal,
            start_container_logs,
            stop_container_logs,
            sftp_get_home_dir,
            sftp_start_upload_batch,
            sftp_start_download_batch,
            sftp_start_move_batch,
            sftp_start_delete_batch,
            sftp_cancel_transfer,
            sftp_get_downloads_dir,
            sftp_pick_files,
            sftp_pick_folder,
            start_compose_pull,
            stop_compose_pull,
            pangolin::get_pangolin_config,
            pangolin::save_pangolin_config,
            pangolin::pangolin_api_request,
            profile_extras::get_profile_extras,
            profile_extras::save_profile_extras
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
