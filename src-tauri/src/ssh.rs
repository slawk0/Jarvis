use std::path::Path;
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use russh::client;
use russh::keys::PrivateKeyWithHashAlg;
use russh_sftp::client::SftpSession;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerStats {
    pub hostname: String,
    pub os: String,
    pub uptime: String,
    pub cpu_usage: f64,
    pub ram_used: u64, // MB
    pub ram_total: u64, // MB
    pub disk_used: u64, // MB
    pub disk_total: u64, // MB
    pub network_rx: u64, // Bytes/s or total
    pub network_tx: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DiskMount {
    pub mount: String,
    pub used_mb: u64,
    pub total_mb: u64,
    pub use_pct: u8,
    pub inode_use_pct: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProcessInfo {
    pub pid: String,
    pub user: String,
    pub cpu: f64,
    pub mem: f64,
    pub command: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExtendedServerStats {
    pub load_1: f64,
    pub load_5: f64,
    pub load_15: f64,
    pub swap_used_mb: u64,
    pub swap_total_mb: u64,
    pub disk_mounts: Vec<DiskMount>,
    pub top_processes: Vec<ProcessInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub permissions: Option<u32>,
    pub modified: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Clone)]
pub struct ClientHandler;

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

#[derive(Clone)]
pub struct SshConnection {
    pub session: Arc<tokio::sync::Mutex<russh::client::Handle<ClientHandler>>>,
    pub host: String,
    pub username: String,
}

impl SshConnection {
    pub async fn connect(
        host: &str,
        port: u16,
        username: &str,
        password: Option<&str>,
        private_key_path: Option<&Path>,
        passphrase: Option<&str>,
    ) -> Result<Self, String> {
        let config = russh::client::Config::default();
        let config = Arc::new(config);
        
        let addr = format!("{}:{}", host, port);
        let socket_addrs = tokio::net::lookup_host(&addr).await
            .map_err(|e| format!("Błąd DNS/Rozwiązywania hosta: {}", e))?
            .collect::<Vec<_>>();
            
        if socket_addrs.is_empty() {
            return Err(format!("Nie znaleziono adresu IP dla hosta: {}", host));
        }
        
        let mut session = russh::client::connect(config, socket_addrs[0], ClientHandler)
            .await
            .map_err(|e| format!("Błąd połączenia SSH: {}", e))?;
            
        let authenticated = if let Some(key_path) = private_key_path {
            let key = russh_keys::load_secret_key(key_path, passphrase)
                .map_err(|e| format!("Błąd ładowania klucza prywatnego (plik {:#?}): {}", key_path, e))?;
            
            let hash_alg = session.best_supported_rsa_hash().await
                .map_err(|e| format!("Błąd uzgadniania hasha RSA: {}", e))?
                .flatten();
                
            let key_with_alg = PrivateKeyWithHashAlg::new(
                Arc::new(key),
                hash_alg
            );
            
            let auth_res = session.authenticate_publickey(username, key_with_alg)
                .await
                .map_err(|e| format!("Błąd autoryzacji kluczem publicznym: {}", e))?;
            matches!(auth_res, russh::client::AuthResult::Success)
        } else if let Some(pass) = password {
            let auth_res = session.authenticate_password(username, pass)
                .await
                .map_err(|e| format!("Błąd autoryzacji hasłem: {}", e))?;
            matches!(auth_res, russh::client::AuthResult::Success)
        } else {
            return Err("Brak poświadczeń do autoryzacji (podaj hasło lub klucz prywatny)".to_string());
        };
        
        if !authenticated {
            return Err("Autoryzacja SSH nie powiodła się".to_string());
        }
        
        Ok(SshConnection {
            session: Arc::new(tokio::sync::Mutex::new(session)),
            host: host.to_string(),
            username: username.to_string(),
        })
    }

    pub async fn exec(&self, cmd: &str) -> Result<(i32, String, String), String> {
        let session = self.session.lock().await;
        let mut channel = session.channel_open_session().await
            .map_err(|e| format!("Błąd otwarcia kanału: {}", e))?;
            
        channel.exec(true, cmd).await
            .map_err(|e| format!("Błąd uruchomienia komendy: {}", e))?;
            
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit_status = 0;
        
        while let Some(msg) = channel.wait().await {
            match msg {
                russh::ChannelMsg::Data { data } => {
                    stdout.extend_from_slice(&data);
                }
                russh::ChannelMsg::ExtendedData { data, ext } => {
                    if ext == 1 {
                        stderr.extend_from_slice(&data);
                    }
                }
                russh::ChannelMsg::ExitStatus { exit_status: code } => {
                    exit_status = code as i32;
                }
                russh::ChannelMsg::Eof => {
                    break;
                }
                _ => {}
            }
        }
        
        let stdout_str = String::from_utf8_lossy(&stdout).into_owned();
        let stderr_str = String::from_utf8_lossy(&stderr).into_owned();
        
        Ok((exit_status, stdout_str, stderr_str))
    }

    pub async fn get_stats(&self) -> Result<ServerStats, String> {
        let script = r#"
        echo "===HOST==="
        hostname; uname -sr; uptime -p
        echo "===CPU==="
        read -r _ user nice system idle iowait irq softirq steal guest guest_nice < /proc/stat
        prev_idle=$((idle + iowait))
        prev_non_idle=$((user + nice + system + irq + softirq + steal))
        prev_total=$((prev_idle + prev_non_idle))
        sleep 0.2
        read -r _ user nice system idle iowait irq softirq steal guest guest_nice < /proc/stat
        idle=$((idle + iowait))
        non_idle=$((user + nice + system + irq + softirq + steal))
        total=$((idle + non_idle))
        total_d=$((total - prev_total))
        idle_d=$((idle - prev_idle))
        if [ "$total_d" -ne 0 ]; then
          echo "$(( (total_d - idle_d) * 100 / total_d ))"
        else
          echo "0"
        fi
        echo "===RAM==="
        free -m | awk 'NR==2{printf "%d %d\n", $3,$2}'
        echo "===DISK==="
        df -BM --output=used,size / | tail -n 1
        echo "===NET==="
        cat /proc/net/dev | grep -E 'eth0|enp|wlan' | awk '{print $1,$2,$10}' | head -n 1
        "#;

        let (_code, stdout, _stderr) = self.exec(script).await?;
        
        // Parsowanie wyjścia
        let lines: Vec<&str> = stdout.lines().collect();
        
        let mut hostname = String::new();
        let mut os = String::new();
        let mut uptime = String::new();
        let mut cpu_usage = 0.0;
        let mut ram_used = 0;
        let mut ram_total = 0;
        let mut disk_used = 0;
        let mut disk_total = 0;
        let mut network_rx = 0;
        let mut network_tx = 0;
        
        let mut section = "";
        let mut host_line_idx = 0;
        
        for line in lines {
            let line = line.trim();
            if line.starts_with("===") {
                section = line;
                continue;
            }
            
            match section {
                "===HOST===" => {
                    if host_line_idx == 0 {
                        hostname = line.to_string();
                    } else if host_line_idx == 1 {
                        os = line.to_string();
                    } else if host_line_idx == 2 {
                        uptime = line.to_string();
                    }
                    host_line_idx += 1;
                }
                "===CPU===" => {
                    cpu_usage = line.parse::<f64>().unwrap_or(0.0);
                }
                "===RAM===" => {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        ram_used = parts[0].parse::<u64>().unwrap_or(0);
                        ram_total = parts[1].parse::<u64>().unwrap_or(0);
                    }
                }
                "===DISK===" => {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        disk_used = parts[0].replace("M", "").parse::<u64>().unwrap_or(0);
                        disk_total = parts[1].replace("M", "").parse::<u64>().unwrap_or(0);
                    }
                }
                "===NET===" => {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        network_rx = parts[1].parse::<u64>().unwrap_or(0);
                        network_tx = parts[2].parse::<u64>().unwrap_or(0);
                    }
                }
                _ => {}
            }
        }
        
        Ok(ServerStats {
            hostname,
            os,
            uptime,
            cpu_usage,
            ram_used,
            ram_total,
            disk_used,
            disk_total,
            network_rx,
            network_tx,
        })
    }

    pub async fn get_extended_stats(&self) -> Result<ExtendedServerStats, String> {
        let script = r#"
        echo "===LOAD==="
        awk '{print $1,$2,$3}' /proc/loadavg
        echo "===SWAP==="
        free -m | awk 'NR==3{printf "%d %d\n", $3,$2}'
        echo "===MOUNTS==="
        df -BM --output=target,used,size,pcent,ipcent 2>/dev/null | tail -n +2 | grep -vE 'tmpfs|snap|loop' | head -20
        echo "===PROCS==="
        ps aux --sort=-%mem 2>/dev/null | awk 'NR>1 && NR<=11 {printf "%s|%s|%.1f|%.1f|%s\n", $2,$1,$3,$4,substr($0,index($0,$11))}'
        "#;

        let (_code, stdout, _stderr) = self.exec(script).await?;

        let mut load_1 = 0.0;
        let mut load_5 = 0.0;
        let mut load_15 = 0.0;
        let mut swap_used_mb = 0u64;
        let mut swap_total_mb = 0u64;
        let mut disk_mounts = Vec::new();
        let mut top_processes = Vec::new();

        let mut section = "";
        for line in stdout.lines() {
            let line = line.trim();
            if line.starts_with("===") {
                section = line;
                continue;
            }
            if line.is_empty() {
                continue;
            }

            match section {
                "===LOAD===" => {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        load_1 = parts[0].parse().unwrap_or(0.0);
                        load_5 = parts[1].parse().unwrap_or(0.0);
                        load_15 = parts[2].parse().unwrap_or(0.0);
                    }
                }
                "===SWAP===" => {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        swap_used_mb = parts[0].parse().unwrap_or(0);
                        swap_total_mb = parts[1].parse().unwrap_or(0);
                    }
                }
                "===MOUNTS===" => {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 5 {
                        let used_mb = parts[1].replace('M', "").parse::<u64>().unwrap_or(0);
                        let total_mb = parts[2].replace('M', "").parse::<u64>().unwrap_or(0);
                        let use_pct = parts[3].replace('%', "").parse::<u8>().unwrap_or(0);
                        let inode_use_pct = parts[4].replace('%', "").parse::<u8>().unwrap_or(0);
                        disk_mounts.push(DiskMount {
                            mount: parts[0].to_string(),
                            used_mb,
                            total_mb,
                            use_pct,
                            inode_use_pct,
                        });
                    }
                }
                "===PROCS===" => {
                    let parts: Vec<&str> = line.splitn(5, '|').collect();
                    if parts.len() >= 5 {
                        top_processes.push(ProcessInfo {
                            pid: parts[0].to_string(),
                            user: parts[1].to_string(),
                            cpu: parts[2].parse().unwrap_or(0.0),
                            mem: parts[3].parse().unwrap_or(0.0),
                            command: parts[4].to_string(),
                        });
                    }
                }
                _ => {}
            }
        }

        Ok(ExtendedServerStats {
            load_1,
            load_5,
            load_15,
            swap_used_mb,
            swap_total_mb,
            disk_mounts,
            top_processes,
        })
    }

    pub async fn get_sftp(&self) -> Result<SftpSession, String> {
        let session = self.session.lock().await;
        let channel = session.channel_open_session().await
            .map_err(|e| format!("Błąd otwarcia kanału SFTP: {}", e))?;
        channel.request_subsystem(true, "sftp").await
            .map_err(|e| format!("Błąd podsystemu SFTP: {}", e))?;
        let sftp = SftpSession::new(channel.into_stream()).await
            .map_err(|e| format!("Błąd inicjalizacji SFTP: {}", e))?;
        Ok(sftp)
    }

    pub async fn sftp_list_dir(&self, dir_path: &str) -> Result<Vec<FileInfo>, String> {
        let sftp = self.get_sftp().await?;
        let entries = sftp.read_dir(dir_path).await
            .map_err(|e| format!("Błąd odczytu katalogu {}: {}", dir_path, e))?;
            
        let mut file_infos = Vec::new();
        for entry in entries {
            let name = entry.file_name().to_string();
            if name == "." || name == ".." || name.is_empty() {
                continue;
            }
            
            let metadata = entry.metadata();
            let is_dir = entry.file_type().is_dir();
            
            let size = metadata.size.unwrap_or(0);
            let permissions = metadata.permissions;
            let modified = metadata.mtime.unwrap_or(0) as u64;
            
            file_infos.push(FileInfo {
                name,
                is_dir,
                size,
                permissions,
                modified,
                path: None,
            });
        }
        
        file_infos.sort_by(|a, b| {
            if a.is_dir != b.is_dir {
                b.is_dir.cmp(&a.is_dir)
            } else {
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            }
        });
        
        Ok(file_infos)
    }

    pub async fn sftp_dir_size(&self, dir_path: &str) -> Result<u64, String> {
        let cmd = crate::du_size::du_folder_cmd(dir_path);
        let (exit_code, stdout, stderr) = self.exec(&cmd).await?;

        if exit_code != 0 {
            let msg = stderr.trim();
            return Err(if msg.is_empty() {
                format!("du zakończyło się kodem {}", exit_code)
            } else {
                msg.to_string()
            });
        }

        let line = stdout.lines().next().unwrap_or("").trim();
        crate::du_size::parse_du_mb_line(line)
    }

    pub async fn sftp_find(
        &self,
        root: &str,
        query: &str,
        hide_hidden: bool,
    ) -> Result<Vec<FileInfo>, String> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        let cmd = crate::sftp_find::find_cmd(root, trimmed, hide_hidden);
        let (exit_code, stdout, stderr) = self.exec(&cmd).await?;

        if exit_code != 0 && stdout.trim().is_empty() {
            let msg = stderr.trim();
            return Err(if msg.is_empty() {
                format!("find zakończyło się kodem {}", exit_code)
            } else {
                msg.to_string()
            });
        }

        Ok(crate::sftp_find::parse_find_output(&stdout))
    }

    pub async fn sftp_read_file(&self, file_path: &str) -> Result<String, String> {
        let sftp = self.get_sftp().await?;
        let mut file = sftp.open_with_flags(file_path, russh_sftp::protocol::OpenFlags::READ)
            .await
            .map_err(|e| format!("Nie można otworzyć pliku {}: {}", file_path, e))?;
            
        let mut content = String::new();
        file.read_to_string(&mut content).await.map_err(|e| format!("Błąd odczytu zawartości pliku: {}", e))?;
        Ok(content)
    }

    pub async fn sftp_write_file(&self, file_path: &str, content: &str) -> Result<(), String> {
        let sftp = self.get_sftp().await?;
        let mut file = sftp.open_with_flags(
            file_path,
            russh_sftp::protocol::OpenFlags::CREATE | russh_sftp::protocol::OpenFlags::TRUNCATE | russh_sftp::protocol::OpenFlags::WRITE,
        ).await.map_err(|e| format!("Nie można utworzyć pliku {}: {}", file_path, e))?;
        
        file.write_all(content.as_bytes()).await.map_err(|e| format!("Błąd zapisu do pliku: {}", e))?;
        file.shutdown().await.map_err(|e| format!("Błąd zamykania pliku: {}", e))?;
        Ok(())
    }

    pub async fn sftp_create_dir(&self, dir_path: &str) -> Result<(), String> {
        let sftp = self.get_sftp().await?;
        sftp.create_dir(dir_path).await.map_err(|e| format!("Nie można utworzyć katalogu: {}", e))?;
        Ok(())
    }

    pub async fn sftp_delete_file(&self, path: &str, is_dir: bool) -> Result<(), String> {
        let sftp = self.get_sftp().await?;
        if is_dir {
            sftp.remove_dir(path).await.map_err(|e| format!("Nie można usunąć katalogu (upewnij się, że jest pusty): {}", e))?;
        } else {
            sftp.remove_file(path).await.map_err(|e| format!("Nie można usunąć pliku: {}", e))?;
        }
        Ok(())
    }

    pub async fn sftp_rename(&self, src: &str, dest: &str) -> Result<(), String> {
        let sftp = self.get_sftp().await?;
        sftp.rename(src, dest).await.map_err(|e| format!("Błąd zmiany nazwy/przeniesienia: {}", e))?;
        Ok(())
    }
}
