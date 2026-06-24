// Install / uninstall / list periodic backup schedules on the remote host.
//
// The frontend builds the full backup script body and the secrets env file
// content (reusing the same command-builders used for manual runs). This module
// stays a thin, validated installer: it writes the script + 600-perm env file
// to root-only locations and merges a marker-tagged line into root's crontab so
// the job runs unattended while the desktop app is closed.

use crate::app_error::AppError;
use crate::du_size::shell_single_quote;
use crate::ssh::SshConnection;
use crate::{get_sudo_password, no_ssh_connection, AppState};
use serde::Serialize;
use tauri::State;

const ENV_DIR: &str = "/etc/jarvis-backups";

fn env_path(id: &str) -> String {
    format!("{}/{}.env", ENV_DIR, id)
}
fn script_path(id: &str) -> String {
    format!("/usr/local/bin/jarvis-backup-{}.sh", id)
}
fn log_path(id: &str) -> String {
    format!("/var/log/jarvis-backup-{}.log", id)
}
fn marker_start(id: &str) -> String {
    format!("# JARVIS-BACKUP:{} START", id)
}
fn marker_end(id: &str) -> String {
    format!("# JARVIS-BACKUP:{} END", id)
}

/// Backup template ids are app-generated (numeric / uuid-like). Restrict to a
/// safe charset so the id can be embedded in file paths and crontab markers.
fn validate_backup_id(id: &str) -> Result<(), AppError> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err(AppError::new("INVALID_BACKUP_ID"));
    }
    Ok(())
}

fn grab_conn(state: &AppState) -> Result<SshConnection, AppError> {
    let guard = state.connection.lock();
    Ok(guard.as_ref().ok_or_else(no_ssh_connection)?.clone())
}

/// Run a command as root using the cached sudo password. Returns
/// (exit_code, stdout, stderr) so callers can tolerate expected failures
/// (e.g. "no crontab for root"). Mirrors the sudo path of `exec_custom_command`.
async fn run_sudo(
    conn: &SshConnection,
    sudo_pass: &str,
    cmd: &str,
) -> Result<(i32, String, String), AppError> {
    let escaped_pass = shell_single_quote(sudo_pass);
    let quoted_cmd = shell_single_quote(cmd);
    let formatted = format!("echo {} | sudo -S -- bash -c {}", escaped_pass, quoted_cmd);
    let (exit_code, stdout, stderr) = conn.exec(&formatted).await?;
    // Redact the password should bash echo it back on error.
    let stderr = stderr.replace(sudo_pass, "[REDACTED]");
    let stdout = stdout.replace(sudo_pass, "[REDACTED]");
    Ok((exit_code, stdout, stderr))
}

/// Run a command as root, erroring on a non-zero exit. Detects an incorrect
/// cached password and surfaces SUDO_PASSWORD_INCORRECT like the main command.
async fn run_sudo_checked(
    conn: &SshConnection,
    sudo_pass: &str,
    cmd: &str,
) -> Result<String, AppError> {
    let (exit_code, stdout, stderr) = run_sudo(conn, sudo_pass, cmd).await?;
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
}

/// Read root's crontab, returning empty string when none exists yet.
async fn read_root_crontab(conn: &SshConnection, sudo_pass: &str) -> Result<String, AppError> {
    let (exit_code, stdout, stderr) = run_sudo(conn, sudo_pass, "crontab -l").await?;
    if exit_code == 0 {
        return Ok(stdout);
    }
    // `crontab -l` exits non-zero with this message when there is no crontab yet.
    if stderr.contains("no crontab") || stdout.contains("no crontab") {
        return Ok(String::new());
    }
    if stderr.contains("incorrect password attempt") || stderr.contains("złe hasło") {
        return Err(AppError::new("SUDO_PASSWORD_INCORRECT"));
    }
    Err(AppError::with_details(
        "REMOTE_COMMAND_FAILED",
        format!("exit={}\nstderr={}\nstdout={}", exit_code, stderr, stdout),
    ))
}

/// Remove the marker block belonging to `id` from a crontab body.
fn strip_block(crontab: &str, id: &str) -> String {
    let start = marker_start(id);
    let end = marker_end(id);
    let mut out: Vec<&str> = Vec::new();
    let mut skipping = false;
    for line in crontab.lines() {
        let trimmed = line.trim_end();
        if trimmed == start {
            skipping = true;
            continue;
        }
        if skipping {
            if trimmed == end {
                skipping = false;
            }
            continue;
        }
        out.push(line);
    }
    out.join("\n")
}

/// Write `content` to `path` (as root) via a single-quoted printf — injection
/// safe for arbitrary content — then chmod it to `mode`.
fn write_file_cmd(path: &str, content: &str, mode: &str) -> String {
    format!(
        "printf '%s' {} | tee {} > /dev/null && chmod {} {}",
        shell_single_quote(content),
        shell_single_quote(path),
        mode,
        shell_single_quote(path),
    )
}

#[derive(Serialize)]
pub struct BackupScheduleInfo {
    pub id: String,
    pub cron: String,
    pub last_log_tail: Option<String>,
}

#[tauri::command]
pub async fn install_backup_schedule(
    state: State<'_, AppState>,
    template_id: String,
    cron: String,
    script_body: String,
    env_content: String,
) -> Result<(), AppError> {
    validate_backup_id(&template_id)?;
    if cron.trim().is_empty() {
        return Err(AppError::new("INVALID_CRON_EXPRESSION"));
    }
    // Reject control characters that would let the cron field break out into a
    // second crontab line; a valid cron expression never contains them.
    if cron.contains('\n') || cron.contains('\r') {
        return Err(AppError::new("INVALID_CRON_EXPRESSION"));
    }

    let conn = grab_conn(&state)?;
    let sudo_pass = get_sudo_password(&state)?;

    // 1. Write the secrets env file (600) and the backup script (755).
    let env_p = env_path(&template_id);
    let script_p = script_path(&template_id);
    let setup = format!(
        "mkdir -p {} && {} && {}",
        shell_single_quote(ENV_DIR),
        write_file_cmd(&env_p, &env_content, "600"),
        write_file_cmd(&script_p, &script_body, "755"),
    );
    run_sudo_checked(&conn, &sudo_pass, &setup).await?;

    // 2. Merge our marker block into root's crontab.
    let current = read_root_crontab(&conn, &sudo_pass).await?;
    let mut body = strip_block(&current, &template_id);
    if !body.is_empty() && !body.ends_with('\n') {
        body.push('\n');
    }
    let cron_line = format!(
        "{} {} >> {} 2>&1",
        cron.trim(),
        script_p,
        log_path(&template_id),
    );
    body.push_str(&marker_start(&template_id));
    body.push('\n');
    body.push_str(&cron_line);
    body.push('\n');
    body.push_str(&marker_end(&template_id));
    body.push('\n');

    // Install the new crontab from stdin.
    let install = format!("printf '%s' {} | crontab -", shell_single_quote(&body));
    run_sudo_checked(&conn, &sudo_pass, &install).await?;
    Ok(())
}

#[tauri::command]
pub async fn uninstall_backup_schedule(
    state: State<'_, AppState>,
    template_id: String,
) -> Result<(), AppError> {
    validate_backup_id(&template_id)?;
    let conn = grab_conn(&state)?;
    let sudo_pass = get_sudo_password(&state)?;

    // Remove the marker block from root's crontab (no-op if absent).
    let current = read_root_crontab(&conn, &sudo_pass).await?;
    let body = strip_block(&current, &template_id);
    let install = if body.trim().is_empty() {
        "crontab -r 2>/dev/null || true".to_string()
    } else {
        let mut b = body;
        if !b.ends_with('\n') {
            b.push('\n');
        }
        format!("printf '%s' {} | crontab -", shell_single_quote(&b))
    };

    // Delete the script + env file alongside the crontab update.
    let cleanup = format!(
        "{} ; rm -f {} {}",
        install,
        shell_single_quote(&script_path(&template_id)),
        shell_single_quote(&env_path(&template_id)),
    );
    run_sudo_checked(&conn, &sudo_pass, &cleanup).await?;
    Ok(())
}

/// Return root's full crontab (read-only viewing in the Cron tab). Empty when
/// root has no crontab yet.
#[tauri::command]
pub async fn get_root_crontab(state: State<'_, AppState>) -> Result<String, AppError> {
    let conn = grab_conn(&state)?;
    let sudo_pass = get_sudo_password(&state)?;
    read_root_crontab(&conn, &sudo_pass).await
}

#[tauri::command]
pub async fn get_backup_schedules(
    state: State<'_, AppState>,
) -> Result<Vec<BackupScheduleInfo>, AppError> {
    let conn = grab_conn(&state)?;
    let sudo_pass = get_sudo_password(&state)?;
    let crontab = read_root_crontab(&conn, &sudo_pass).await?;

    let mut out = Vec::new();
    let lines: Vec<&str> = crontab.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim_end();
        if let Some(rest) = line.strip_prefix("# JARVIS-BACKUP:") {
            if let Some(id) = rest.strip_suffix(" START") {
                // The next non-marker line is the cron line.
                let mut cron = String::new();
                let mut j = i + 1;
                while j < lines.len() {
                    let l = lines[j].trim_end();
                    if l == marker_end(id) {
                        break;
                    }
                    if !l.is_empty() {
                        // Drop the trailing `<script> >> <log> 2>&1` to keep just the schedule.
                        cron = l
                            .split(&script_path(id))
                            .next()
                            .unwrap_or(l)
                            .trim()
                            .to_string();
                    }
                    j += 1;
                }
                // Best-effort: tail the log file for a status line on the card.
                let tail_cmd = format!("tail -n 5 {} 2>/dev/null || true", shell_single_quote(&log_path(id)));
                let last_log_tail = run_sudo(&conn, &sudo_pass, &tail_cmd)
                    .await
                    .ok()
                    .map(|(_, stdout, _)| stdout.trim().to_string())
                    .filter(|s| !s.is_empty());
                out.push(BackupScheduleInfo {
                    id: id.to_string(),
                    cron,
                    last_log_tail,
                });
                i = j;
            }
        }
        i += 1;
    }
    Ok(out)
}
