mod ssh;

use parking_lot::Mutex;
use std::sync::Arc;
use std::fs;
use std::path::PathBuf;
use tauri::{Manager, Emitter, State, AppHandle};
use serde::{Serialize, Deserialize};
use keyring::Entry;
use ssh::{SshConnection, ServerStats, FileInfo};
use tokio::sync::mpsc;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Profile {
    pub id: String,
    pub label: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: String, // "password" | "key"
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
    pub terminal_tx: Arc<Mutex<Option<mpsc::Sender<String>>>>,
}

// Pomocnicza funkcja do pobierania ścieżki pliku profili
fn get_profiles_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let mut path = app_handle.path().app_config_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&path).map_err(|e| format!("Nie można utworzyć katalogu konfiguracji: {}", e))?;
    path.push("profiles.json");
    Ok(path)
}

#[tauri::command]
fn get_profiles(app_handle: AppHandle) -> Result<Vec<Profile>, String> {
    let path = get_profiles_path(&app_handle)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path).map_err(|e| format!("Błąd odczytu profili: {}", e))?;
    let profiles: Vec<Profile> = serde_json::from_str(&content).unwrap_or_else(|_| Vec::new());
    Ok(profiles)
}

#[tauri::command]
fn save_profile(
    app_handle: AppHandle,
    profile: Profile,
    password: Option<String>,
    key_passphrase: Option<String>,
) -> Result<(), String> {
    // 1. Zapis poświadczeń do Keyringa
    let keyring_service = "JarvisServerManager";
    
    if let Some(pass) = password {
        let entry = Entry::new(keyring_service, &format!("{}_pass", profile.id))
            .map_err(|e| format!("Błąd inicjalizacji Keyringa: {}", e))?;
        entry.set_password(&pass).map_err(|e| format!("Błąd zapisu hasła w Keyringu: {}", e))?;
    }
    
    if let Some(passphrase) = key_passphrase {
        let entry = Entry::new(keyring_service, &format!("{}_passphrase", profile.id))
            .map_err(|e| format!("Błąd inicjalizacji Keyringa: {}", e))?;
        entry.set_password(&passphrase).map_err(|e| format!("Błąd zapisu hasła klucza w Keyringu: {}", e))?;
    }

    // 2. Zapis profilu do pliku JSON
    let path = get_profiles_path(&app_handle)?;
    let mut profiles = get_profiles(app_handle)?;
    
    // Jeśli profil już istnieje, nadpisujemy go, w przeciwnym razie dodajemy nowy
    if let Some(pos) = profiles.iter().position(|p| p.id == profile.id) {
        profiles[pos] = profile;
    } else {
        profiles.push(profile);
    }
    
    let content = serde_json::to_string_pretty(&profiles).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| format!("Błąd zapisu pliku profili: {}", e))?;
    
    Ok(())
}

#[tauri::command]
fn delete_profile(app_handle: AppHandle, id: String) -> Result<(), String> {
    let keyring_service = "JarvisServerManager";
    
    // Usuwanie poświadczeń z Keyringa
    if let Ok(entry) = Entry::new(keyring_service, &format!("{}_pass", id)) {
        entry.delete_password().ok();
    }
    if let Ok(entry) = Entry::new(keyring_service, &format!("{}_passphrase", id)) {
        entry.delete_password().ok();
    }

    // Usuwanie profilu z JSON
    let path = get_profiles_path(&app_handle)?;
    let mut profiles = get_profiles(app_handle)?;
    profiles.retain(|p| p.id != id);
    
    let content = serde_json::to_string_pretty(&profiles).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| format!("Błąd zapisu pliku profili: {}", e))?;
    
    Ok(())
}

#[tauri::command]
async fn connect_ssh(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    profile_id: String,
) -> Result<ServerStats, String> {
    // Rozłącz poprzednie połączenie
    disconnect_ssh(state.clone()).await.ok();

    // 1. Pobierz profil
    let profiles = get_profiles(app_handle)?;
    let profile = profiles.iter().find(|p| p.id == profile_id)
        .ok_or_else(|| "Profil nie istnieje".to_string())?;

    // 2. Pobierz poświadczenia z Keyringa
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

    // 3. Połącz przez SSH
    let conn = SshConnection::connect(
        &profile.host,
        profile.port,
        &profile.username,
        password.as_deref(),
        private_key_path,
        passphrase.as_deref()
    ).await?;

    // 4. Pobierz wstępne statystyki serwera
    let stats = conn.get_stats().await?;

    // Zapisz poświadczenia w RAM do ponownego wykorzystania (np. do terminala)
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
    
    // Resetuj sudo_password
    *state.sudo_password.lock() = None;

    Ok(stats)
}

#[tauri::command]
async fn disconnect_ssh(state: State<'_, AppState>) -> Result<(), String> {
    // Zamknij połączenie SSH
    *state.connection.lock() = None;
    *state.ssh_creds.lock() = None;
    *state.sudo_password.lock() = None;
    
    // Zamknij terminal
    let mut tx_guard = state.terminal_tx.lock();
    if tx_guard.is_some() {
        *tx_guard = None;
    }

    Ok(())
}

#[tauri::command]
async fn get_server_stats(state: State<'_, AppState>) -> Result<ServerStats, String> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or("Brak aktywnego połączenia SSH")?.clone()
    };
    conn.get_stats().await
}

#[tauri::command]
async fn exec_custom_command(
    state: State<'_, AppState>,
    cmd: String,
    use_sudo: bool,
) -> Result<String, String> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or("Brak aktywnego połączenia SSH")?.clone()
    };

    if use_sudo {
        let sudo_pass = {
            let sudo_pass_guard = state.sudo_password.lock();
            sudo_pass_guard.as_ref().ok_or("SUDO_PASSWORD_REQUIRED")?.clone()
        };
        
        // Wykonanie komendy przez sudo -S (przekazanie hasła przez stdin)
        let formatted_cmd = format!("echo '{}' | sudo -S -- {}", sudo_pass, cmd);
        let (exit_code, stdout, stderr) = conn.exec(&formatted_cmd).await?;
        
        if exit_code != 0 {
            // Jeśli hasło sudo było niepoprawne
            if stderr.contains("incorrect password attempt") || stderr.contains("złe hasło") {
                return Err("SUDO_PASSWORD_INCORRECT".to_string());
            }
            return Err(format!("Błąd [kod {}]: {}\n{}", exit_code, stderr, stdout));
        }
        Ok(stdout)
    } else {
        let (exit_code, stdout, stderr) = conn.exec(&cmd).await?;
        if exit_code != 0 {
            return Err(format!("Błąd [kod {}]: {}\n{}", exit_code, stderr, stdout));
        }
        Ok(stdout)
    }
}

#[tauri::command]
async fn set_sudo_password(state: State<'_, AppState>, password: String) -> Result<(), String> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or("Brak aktywnego połączenia SSH")?.clone()
    };
    
    // Zweryfikuj hasło sudo wykonując prostą komendę (np. sudo -S id)
    let test_cmd = format!("echo '{}' | sudo -S id", password);
    let (exit_code, _stdout, stderr) = conn.exec(&test_cmd).await?;
    
    if exit_code != 0 {
        if stderr.contains("incorrect password attempt") || stderr.contains("złe hasło") {
            return Err("Niepoprawne hasło sudo".to_string());
        }
        return Err(format!("Błąd weryfikacji sudo: {}", stderr));
    }
    
    *state.sudo_password.lock() = Some(password);
    Ok(())
}

#[tauri::command]
fn has_sudo_password(state: State<'_, AppState>) -> bool {
    state.sudo_password.lock().is_some()
}

// SFTP Commands
#[tauri::command]
async fn sftp_list(state: State<'_, AppState>, path: String) -> Result<Vec<FileInfo>, String> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or("Brak aktywnego połączenia SSH")?.clone()
    };
    conn.sftp_list_dir(&path).await
}

#[tauri::command]
async fn sftp_read(state: State<'_, AppState>, path: String) -> Result<String, String> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or("Brak aktywnego połączenia SSH")?.clone()
    };
    conn.sftp_read_file(&path).await
}

#[tauri::command]
async fn sftp_write(state: State<'_, AppState>, path: String, content: String) -> Result<(), String> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or("Brak aktywnego połączenia SSH")?.clone()
    };
    conn.sftp_write_file(&path, &content).await
}

#[tauri::command]
async fn sftp_create_dir(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or("Brak aktywnego połączenia SSH")?.clone()
    };
    conn.sftp_create_dir(&path).await
}

#[tauri::command]
async fn sftp_delete(state: State<'_, AppState>, path: String, is_dir: bool) -> Result<(), String> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or("Brak aktywnego połączenia SSH")?.clone()
    };
    conn.sftp_delete_file(&path, is_dir).await
}

#[tauri::command]
async fn sftp_rename(state: State<'_, AppState>, src: String, dest: String) -> Result<(), String> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or("Brak aktywnego połączenia SSH")?.clone()
    };
    conn.sftp_rename(&src, &dest).await
}

// Terminal Commands
#[tauri::command]
async fn start_terminal(
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    // Zamknij poprzedni terminal jeśli istnieje
    let mut tx_guard = state.terminal_tx.lock();
    if tx_guard.is_some() {
        *tx_guard = None;
    }

    let creds_guard = state.ssh_creds.lock();
    let creds = creds_guard.as_ref().ok_or("Brak aktywnego połączenia SSH")?.clone();

    // Utwórz nowy kanał komunikacyjny dla wejścia terminala
    let (tx, mut rx) = mpsc::channel::<String>(100);
    *tx_guard = Some(tx);

    let app_handle_clone = app_handle.clone();
    
    // Spawnowanie dedykowanego asynchronicznego zadania tokio
    tokio::spawn(async move {
        let run_terminal = async {
            let private_key_path = creds.private_key.as_ref().map(std::path::Path::new);
            let conn = SshConnection::connect(
                &creds.host,
                creds.port,
                &creds.username,
                creds.password.as_deref(),
                private_key_path,
                creds.passphrase.as_deref()
            ).await?;
            
            let session = conn.session.lock().await;
            let mut channel = session.channel_open_session().await
                .map_err(|e| format!("Błąd kanału: {}", e))?;
            
            channel.request_pty(true, "xterm-256color", 80, 24, 0, 0, &[])
                .await
                .map_err(|e| format!("Błąd PTY: {}", e))?;
            
            channel.request_shell(true)
                .await
                .map_err(|e| format!("Błąd Shell: {}", e))?;

            let app_handle_read = app_handle_clone.clone();
            
            loop {
                tokio::select! {
                    msg = channel.wait() => {
                        match msg {
                            Some(russh::ChannelMsg::Data { data }) => {
                                let stdout_str = String::from_utf8_lossy(&data).to_string();
                                app_handle_read.emit("terminal-stdout", stdout_str).ok();
                            }
                            Some(russh::ChannelMsg::ExtendedData { data, ext }) => {
                                if ext == 1 {
                                    let stderr_str = String::from_utf8_lossy(&data).to_string();
                                    app_handle_read.emit("terminal-stdout", stderr_str).ok();
                                }
                            }
                            Some(russh::ChannelMsg::ExitStatus { exit_status }) => {
                                app_handle_read.emit("terminal-stdout", format!("\r\n[Połączenie terminala zamknięte, kod: {}]\r\n", exit_status)).ok();
                                break;
                            }
                            Some(russh::ChannelMsg::Eof) => {
                                app_handle_read.emit("terminal-stdout", "\r\n[Połączenie terminala zamknięte]\r\n".to_string()).ok();
                                break;
                            }
                            None => {
                                break;
                            }
                            _ => {}
                        }
                    }
                    input_opt = rx.recv() => {
                        match input_opt {
                            Some(input_data) => {
                                if let Err(e) = channel.data(input_data.as_bytes()).await {
                                    app_handle_read.emit("terminal-stdout", format!("\r\n[Błąd zapisu do terminala: {}]\r\n", e)).ok();
                                    break;
                                }
                            }
                            None => {
                                break;
                            }
                        }
                    }
                }
            }
            
            Ok::<(), String>(())
        };

        if let Err(e) = run_terminal.await {
            app_handle_clone.emit("terminal-stdout", format!("\r\n[Błąd sesji SSH terminala: {}]\r\n", e)).ok();
        }
    });

    Ok(())
}

#[tauri::command]
fn send_terminal_input(state: State<'_, AppState>, input: String) -> Result<(), String> {
    let tx_guard = state.terminal_tx.lock();
    if let Some(ref tx) = *tx_guard {
        let tx = tx.clone();
        tokio::spawn(async move {
            tx.send(input).await.ok();
        });
        Ok(())
    } else {
        Err("Terminal nie jest uruchomiony".to_string())
    }
}

#[tauri::command]
fn open_external_terminal(app_handle: AppHandle, profile_id: String) -> Result<(), String> {
    let profiles = get_profiles(app_handle)?;
    let profile = profiles.iter().find(|p| p.id == profile_id)
        .ok_or_else(|| "Profil nie istnieje".to_string())?;

    // Sprawdź czy system to Windows
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        
        let ssh_args = if profile.auth_type == "key" {
            if let Some(ref key_path) = profile.key_path {
                format!("ssh -i \"{}\" -p {} {}@{}", key_path, profile.port, profile.username, profile.host)
            } else {
                format!("ssh -p {} {}@{}", profile.port, profile.username, profile.host)
            }
        } else {
            format!("ssh -p {} {}@{}", profile.port, profile.username, profile.host)
        };

        // Próbujemy otworzyć w Windows Terminal, a jeśli nie ma, to w cmd.exe
        let wt_res = Command::new("wt")
            .arg("cmd.exe")
            .arg("/k")
            .arg(&ssh_args)
            .spawn();

        if wt_res.is_err() {
            // Fallback do tradycyjnego cmd start
            Command::new("cmd.exe")
                .arg("/c")
                .arg("start")
                .arg("cmd.exe")
                .arg("/k")
                .arg(&ssh_args)
                .spawn()
                .map_err(|e| format!("Nie można otworzyć terminala CMD: {}", e))?;
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        // Obsługa innych OS w razie czego (np. MacOS Terminal, Linux xterm)
        return Err("Zewnętrzny terminal jest wspierany obecnie na systemie Windows".to_string());
    }

    Ok(())
}

#[tauri::command]
async fn sftp_get_home_dir(state: State<'_, AppState>) -> Result<String, String> {
    let conn = {
        let conn_guard = state.connection.lock();
        conn_guard.as_ref().ok_or("Brak aktywnego połączenia SSH")?.clone()
    };
    let sftp = conn.get_sftp().await?;
    let path = sftp.canonicalize(".").await
        .map_err(|e| format!("Błąd pobierania katalogu domowego: {}", e))?;
    Ok(path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            connection: Arc::new(Mutex::new(None)),
            sudo_password: Arc::new(Mutex::new(None)),
            ssh_creds: Arc::new(Mutex::new(None)),
            terminal_tx: Arc::new(Mutex::new(None)),
        })
        .invoke_handler(tauri::generate_handler![
            get_profiles,
            save_profile,
            delete_profile,
            connect_ssh,
            disconnect_ssh,
            get_server_stats,
            exec_custom_command,
            set_sudo_password,
            has_sudo_password,
            sftp_list,
            sftp_read,
            sftp_write,
            sftp_create_dir,
            sftp_delete,
            sftp_rename,
            start_terminal,
            send_terminal_input,
            open_external_terminal,
            sftp_get_home_dir
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
