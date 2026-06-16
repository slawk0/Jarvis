use crate::app_error::AppError;
use crate::ssh::SshConnection;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::fs::{self, File};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Mutex, Semaphore};

const CHUNK_SIZE: usize = 512 * 1024;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(200);
const MAX_RETRIES: u32 = 3;
const RETRY_DELAYS: [u64; 3] = [1, 3, 9];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TransferKind {
    Upload,
    Download,
    Move,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TransferStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSummary {
    pub completed: u32,
    pub failed: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftpTransferEvent {
    pub job_id: String,
    pub file_name: String,
    pub kind: TransferKind,
    pub status: TransferStatus,
    pub bytes_done: u64,
    pub total_bytes: u64,
    pub speed_bps: u64,
    pub error: Option<String>,
    pub remote_path: String,
    pub local_path: Option<String>,
    pub dest_path: Option<String>,
    pub is_dir: bool,
    pub batch: BatchSummary,
}

#[derive(Debug, Clone)]
pub(crate) struct InternalJob {
    id: String,
    kind: TransferKind,
    local_path: Option<String>,
    remote_path: String,
    total_bytes: u64,
    is_dir: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UploadItem {
    pub local_path: String,
    pub remote_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MoveItem {
    pub src: String,
    pub dest: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeleteItem {
    pub path: String,
    pub is_dir: bool,
}

pub fn get_downloads_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            let dir = PathBuf::from(userprofile).join("Downloads");
            if dir.exists() {
                return dir;
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(home) = std::env::var("HOME") {
            let dir = PathBuf::from(&home).join("Downloads");
            if dir.exists() {
                return dir;
            }
            return PathBuf::from(home);
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn file_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_string()
}

fn emit_event(app: &AppHandle, event: SftpTransferEvent) {
    app.emit("sftp-transfer-event", event).ok();
}

fn job_event(
    job: &InternalJob,
    status: TransferStatus,
    bytes_done: u64,
    total_bytes: u64,
    speed_bps: u64,
    error: Option<String>,
    batch: &BatchSummary,
) -> SftpTransferEvent {
    SftpTransferEvent {
        job_id: job.id.clone(),
        file_name: file_name_from_path(&job.remote_path),
        kind: job.kind.clone(),
        status,
        bytes_done,
        total_bytes,
        speed_bps,
        error,
        remote_path: job.remote_path.clone(),
        local_path: job.local_path.clone(),
        dest_path: if job.kind == TransferKind::Move {
            job.local_path.clone()
        } else {
            None
        },
        is_dir: job.is_dir,
        batch: batch.clone(),
    }
}

fn error_payload(err: &AppError) -> String {
    err.to_json()
}

async fn sleep_backoff(attempt: u32) {
    let idx = (attempt as usize).min(RETRY_DELAYS.len() - 1);
    tokio::time::sleep(Duration::from_secs(RETRY_DELAYS[idx])).await;
}

pub async fn open_sftp(conn: &SshConnection) -> Result<SftpSession, AppError> {
    conn.get_sftp().await
}

pub async fn sftp_stat_size(sftp: &SftpSession, path: &str) -> Result<u64, AppError> {
    let meta = sftp
        .metadata(path)
        .await
        .map_err(|e| AppError::with_details("SFTP_METADATA_FAILED", format!("{}: {}", path, e)))?;
    Ok(meta.size.unwrap_or(0))
}

pub async fn sftp_ensure_parent_dirs(sftp: &SftpSession, remote_path: &str) -> Result<(), AppError> {
    let path = Path::new(remote_path);
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    let parent_str = parent.to_string_lossy();
    if parent_str.is_empty() || parent_str == "/" {
        return Ok(());
    }

    let mut current = String::new();
    for part in parent_str.split('/').filter(|p| !p.is_empty()) {
        if current.is_empty() {
            current = format!("/{}", part);
        } else {
            current = format!("{}/{}", current, part);
        }
        if sftp.metadata(&current).await.is_err() {
            sftp.create_dir(&current)
                .await
                .map_err(|e| AppError::with_details("SFTP_DIR_CREATE_FAILED", format!("{}: {}", current, e)))?;
        }
    }
    Ok(())
}

pub async fn sftp_upload_stream(
    sftp: &SftpSession,
    local_path: &str,
    remote_path: &str,
    cancel: &AtomicBool,
    on_progress: impl Fn(u64, u64, u64),
) -> Result<(), AppError> {
    let local_meta = fs::metadata(local_path)
        .await
        .map_err(|e| AppError::with_details("LOCAL_FILE_READ_FAILED", format!("{}: {}", local_path, e)))?;
    let total = local_meta.len();

    sftp_ensure_parent_dirs(sftp, remote_path).await?;

    let part_path = format!("{}.part", remote_path);
    let _ = sftp.remove_file(&part_path).await;

    let mut local_file = File::open(local_path)
        .await
        .map_err(|e| AppError::with_details("LOCAL_FILE_OPEN_FAILED", format!("{}: {}", local_path, e)))?;

    let mut remote_file = sftp
        .open_with_flags(
            &part_path,
            OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
        )
        .await
        .map_err(|e| AppError::with_details("REMOTE_FILE_CREATE_FAILED", format!("{}: {}", part_path, e)))?;

    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut done: u64 = 0;
    let start = Instant::now();
    let mut last_emit = Instant::now();

    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = sftp.remove_file(&part_path).await;
            return Err(AppError::new(AppError::TRANSFER_CANCELLED));
        }

        let n = local_file
            .read(&mut buf)
            .await
            .map_err(|e| AppError::with_details("LOCAL_READ_FAILED", e.to_string()))?;
        if n == 0 {
            break;
        }

        remote_file
            .write_all(&buf[..n])
            .await
            .map_err(|e| AppError::with_details("REMOTE_WRITE_FAILED", e.to_string()))?;
        done += n as u64;

        if last_emit.elapsed() >= PROGRESS_INTERVAL {
            let elapsed = start.elapsed().as_secs_f64().max(0.001);
            let speed = (done as f64 / elapsed) as u64;
            on_progress(done, total, speed);
            last_emit = Instant::now();
        }
    }

    remote_file
        .shutdown()
        .await
        .map_err(|e| AppError::with_details("REMOTE_FILE_CLOSE_FAILED", e.to_string()))?;

    let remote_size = sftp_stat_size(sftp, &part_path).await?;
    if remote_size != total {
        let _ = sftp.remove_file(&part_path).await;
        return Err(AppError::with_details(
            "TRANSFER_VERIFY_FAILED",
            format!("expected {} bytes, got {}", total, remote_size),
        ));
    }

    sftp.rename(&part_path, remote_path)
        .await
        .map_err(|e| AppError::with_details("UPLOAD_FINALIZE_FAILED", e.to_string()))?;

    let elapsed = start.elapsed().as_secs_f64().max(0.001);
    on_progress(total, total, (total as f64 / elapsed) as u64);
    Ok(())
}

pub async fn sftp_download_stream(
    sftp: &SftpSession,
    remote_path: &str,
    local_path: &str,
    cancel: &AtomicBool,
    on_progress: impl Fn(u64, u64, u64),
) -> Result<(), AppError> {
    let total = sftp_stat_size(sftp, remote_path).await?;

    if let Some(parent) = Path::new(local_path).parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::with_details("LOCAL_DIR_CREATE_FAILED", e.to_string()))?;
    }

    let part_path = format!("{}.part", local_path);
    let _ = fs::remove_file(&part_path).await;

    let mut remote_file = sftp
        .open_with_flags(remote_path, OpenFlags::READ)
        .await
        .map_err(|e| AppError::with_details("REMOTE_FILE_OPEN_FAILED", format!("{}: {}", remote_path, e)))?;

    let mut local_file = File::create(&part_path)
        .await
        .map_err(|e| AppError::with_details("LOCAL_FILE_CREATE_FAILED", format!("{}: {}", part_path, e)))?;

    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut done: u64 = 0;
    let start = Instant::now();
    let mut last_emit = Instant::now();

    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = fs::remove_file(&part_path).await;
            return Err(AppError::new(AppError::TRANSFER_CANCELLED));
        }

        let n = remote_file
            .read(&mut buf)
            .await
            .map_err(|e| AppError::with_details("REMOTE_READ_FAILED", e.to_string()))?;
        if n == 0 {
            break;
        }

        local_file
            .write_all(&buf[..n])
            .await
            .map_err(|e| AppError::with_details("LOCAL_WRITE_FAILED", e.to_string()))?;
        done += n as u64;

        if last_emit.elapsed() >= PROGRESS_INTERVAL {
            let elapsed = start.elapsed().as_secs_f64().max(0.001);
            let speed = (done as f64 / elapsed) as u64;
            on_progress(done, total, speed);
            last_emit = Instant::now();
        }
    }

    local_file
        .flush()
        .await
        .map_err(|e| AppError::with_details("LOCAL_FLUSH_FAILED", e.to_string()))?;
    drop(local_file);

    let local_meta = fs::metadata(&part_path)
        .await
        .map_err(|e| AppError::with_details("LOCAL_FILE_READ_FAILED", e.to_string()))?;
    if local_meta.len() != total {
        let _ = fs::remove_file(&part_path).await;
        return Err(AppError::with_details(
            "TRANSFER_VERIFY_FAILED",
            format!("expected {} bytes, got {}", total, local_meta.len()),
        ));
    }

    fs::rename(&part_path, local_path)
        .await
        .map_err(|e| AppError::with_details("DOWNLOAD_FINALIZE_FAILED", e.to_string()))?;

    let elapsed = start.elapsed().as_secs_f64().max(0.001);
    on_progress(total, total, (total as f64 / elapsed) as u64);
    Ok(())
}

pub async fn sftp_delete_recursive(sftp: &SftpSession, path: &str) -> Result<(), AppError> {
    let entries = sftp
        .read_dir(path)
        .await
        .map_err(|e| AppError::with_details("SFTP_DIR_READ_FAILED", format!("{}: {}", path, e)))?;

    for entry in entries {
        let name = entry.file_name();
        if name == "." || name == ".." || name.is_empty() {
            continue;
        }
        let child = if path.ends_with('/') {
            format!("{}{}", path, name)
        } else {
            format!("{}/{}", path, name)
        };
        if entry.file_type().is_dir() {
            Box::pin(sftp_delete_recursive(sftp, &child)).await?;
        } else {
            sftp.remove_file(&child)
                .await
                .map_err(|e| AppError::with_details("SFTP_FILE_DELETE_FAILED", format!("{}: {}", child, e)))?;
        }
    }

    sftp.remove_dir(path)
        .await
        .map_err(|e| AppError::with_details("SFTP_DIR_DELETE_FAILED", format!("{}: {}", path, e)))?;
    Ok(())
}

pub async fn sftp_move_path(sftp: &SftpSession, src: &str, dest: &str) -> Result<(), AppError> {
    if let Err(e) = sftp.rename(src, dest).await {
        let err = e.to_string();
        if sftp
            .metadata(src)
            .await
            .map(|m| m.is_dir())
            .unwrap_or(false)
        {
            sftp_copy_recursive(sftp, src, dest).await?;
            return sftp_delete_recursive(sftp, src).await;
        }
        return Err(AppError::with_details(
            "SFTP_MOVE_FAILED",
            format!("{} -> {}: {}", src, dest, err),
        ));
    }
    Ok(())
}

async fn sftp_copy_file(sftp: &SftpSession, src: &str, dest: &str) -> Result<(), AppError> {
    sftp_ensure_parent_dirs(sftp, dest).await?;
    let total = sftp_stat_size(sftp, src).await?;
    let mut remote_src = sftp
        .open_with_flags(src, OpenFlags::READ)
        .await
        .map_err(|e| AppError::with_details("REMOTE_FILE_OPEN_FAILED", format!("{}: {}", src, e)))?;
    let mut remote_dest = sftp
        .open_with_flags(
            dest,
            OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
        )
        .await
        .map_err(|e| AppError::with_details("REMOTE_FILE_CREATE_FAILED", format!("{}: {}", dest, e)))?;

    let mut buf = vec![0u8; CHUNK_SIZE];
    loop {
        let n = remote_src
            .read(&mut buf)
            .await
            .map_err(|e| AppError::with_details("SFTP_COPY_FAILED", format!("{}: {}", src, e)))?;
        if n == 0 {
            break;
        }
        remote_dest
            .write_all(&buf[..n])
            .await
            .map_err(|e| AppError::with_details("SFTP_COPY_FAILED", format!("{}: {}", dest, e)))?;
    }
    remote_dest
        .shutdown()
        .await
        .map_err(|e| AppError::with_details("REMOTE_FILE_CLOSE_FAILED", e.to_string()))?;

    let dest_size = sftp_stat_size(sftp, dest).await?;
    if dest_size != total {
        return Err(AppError::with_details(
            "SFTP_COPY_VERIFY_FAILED",
            format!("{} vs {} bytes", total, dest_size),
        ));
    }
    Ok(())
}

pub async fn sftp_copy_recursive(sftp: &SftpSession, src: &str, dest: &str) -> Result<(), AppError> {
    let meta = sftp
        .metadata(src)
        .await
        .map_err(|e| AppError::with_details("SFTP_METADATA_FAILED", format!("{}: {}", src, e)))?;

    if !meta.is_dir() {
        return sftp_copy_file(sftp, src, dest).await;
    }

    sftp.create_dir(dest)
        .await
        .map_err(|e| AppError::with_details("SFTP_DIR_CREATE_FAILED", format!("{}: {}", dest, e)))?;

    let entries = sftp
        .read_dir(src)
        .await
        .map_err(|e| AppError::with_details("SFTP_DIR_READ_FAILED", format!("{}: {}", src, e)))?;

    for entry in entries {
        let name = entry.file_name();
        if name == "." || name == ".." || name.is_empty() {
            continue;
        }
        let src_child = if src.ends_with('/') {
            format!("{}{}", src, name)
        } else {
            format!("{}/{}", src, name)
        };
        let dest_child = if dest.ends_with('/') {
            format!("{}{}", dest, name)
        } else {
            format!("{}/{}", dest, name)
        };
        Box::pin(sftp_copy_recursive(sftp, &src_child, &dest_child)).await?;
    }
    Ok(())
}

pub async fn collect_remote_files(
    sftp: &SftpSession,
    remote_path: &str,
    local_base: &Path,
    remote_base: &str,
) -> Result<Vec<InternalJob>, AppError> {
    let meta = sftp
        .metadata(remote_path)
        .await
        .map_err(|e| AppError::with_details("SFTP_METADATA_FAILED", format!("{}: {}", remote_path, e)))?;

    if !meta.is_dir() {
        let rel = remote_path
            .strip_prefix(remote_base)
            .unwrap_or(remote_path)
            .trim_start_matches('/');
        let local = if rel.is_empty() {
            local_base.join(file_name_from_path(remote_path))
        } else {
            local_base.join(rel)
        };
        return Ok(vec![InternalJob {
            id: String::new(),
            kind: TransferKind::Download,
            local_path: Some(local.to_string_lossy().to_string()),
            remote_path: remote_path.to_string(),
            total_bytes: meta.size.unwrap_or(0),
            is_dir: false,
        }]);
    }

    let mut jobs = Vec::new();
    let entries = sftp
        .read_dir(remote_path)
        .await
        .map_err(|e| AppError::with_details("SFTP_DIR_READ_FAILED", format!("{}: {}", remote_path, e)))?;

    for entry in entries {
        let name = entry.file_name();
        if name == "." || name == ".." || name.is_empty() {
            continue;
        }
        let child_remote = if remote_path.ends_with('/') {
            format!("{}{}", remote_path, name)
        } else {
            format!("{}/{}", remote_path, name)
        };
        let mut child_jobs =
            Box::pin(collect_remote_files(sftp, &child_remote, local_base, remote_base)).await?;
        jobs.append(&mut child_jobs);
    }
    Ok(jobs)
}

pub fn collect_local_files(
    local_paths: &[String],
    remote_dir: &str,
) -> Result<Vec<InternalJob>, AppError> {
    let mut jobs = Vec::new();

    for local_path in local_paths {
        let path = Path::new(local_path);
        if !path.exists() {
            return Err(AppError::with_details("LOCAL_PATH_NOT_FOUND", local_path.clone()));
        }

        if path.is_file() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| AppError::with_details("INVALID_LOCAL_PATH_NAME", local_path.clone()))?;
            let remote = if remote_dir.ends_with('/') {
                format!("{}{}", remote_dir, name)
            } else {
                format!("{}/{}", remote_dir, name)
            };
            let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            jobs.push(InternalJob {
                id: String::new(),
                kind: TransferKind::Upload,
                local_path: Some(local_path.clone()),
                remote_path: remote,
                total_bytes: size,
                is_dir: false,
            });
        } else if path.is_dir() {
            collect_local_dir(path, path, remote_dir, &mut jobs)?;
        }
    }
    Ok(jobs)
}

fn collect_local_dir(
    root: &Path,
    current: &Path,
    remote_dir: &str,
    jobs: &mut Vec<InternalJob>,
) -> Result<(), AppError> {
    for entry in std::fs::read_dir(current).map_err(|e| {
        AppError::with_details("LOCAL_DIR_READ_FAILED", e.to_string())
    })? {
        let entry = entry.map_err(|e| AppError::with_details("LOCAL_DIR_READ_FAILED", e.to_string()))?;
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .map_err(|e| AppError::with_details("LOCAL_DIR_READ_FAILED", e.to_string()))?
            .to_string_lossy()
            .replace('\\', "/");
        let remote = if rel.is_empty() {
            remote_dir.to_string()
        } else if remote_dir.ends_with('/') {
            format!("{}{}", remote_dir, rel)
        } else {
            format!("{}/{}", remote_dir, rel)
        };

        if path.is_dir() {
            collect_local_dir(root, &path, remote_dir, jobs)?;
        } else {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            jobs.push(InternalJob {
                id: String::new(),
                kind: TransferKind::Upload,
                local_path: Some(path.to_string_lossy().to_string()),
                remote_path: remote,
                total_bytes: size,
                is_dir: false,
            });
        }
    }
    Ok(())
}

pub struct TransferRunner {
    cancel: Arc<AtomicBool>,
}

impl TransferRunner {
    pub fn new(cancel: Arc<AtomicBool>) -> Self {
        Self { cancel }
    }

    pub async fn run_batch(
        &self,
        conn: SshConnection,
        app: AppHandle,
        mut jobs: Vec<InternalJob>,
    ) {
        let total = jobs.len() as u32;
        let counters = Arc::new(Mutex::new((0u32, 0u32)));

        for (idx, job) in jobs.iter_mut().enumerate() {
            job.id = format!("job-{}", idx);
        }

        let semaphore = Arc::new(Semaphore::new(2));
        let conn = Arc::new(conn);
        let app = Arc::new(app);
        let cancel = self.cancel.clone();

        let mut handles = Vec::new();

        for job in jobs {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let conn = conn.clone();
            let app = app.clone();
            let cancel = cancel.clone();
            let counters = counters.clone();
            let job = job;

            let batch_snapshot = {
                let (c, f) = *counters.lock().await;
                BatchSummary {
                    completed: c,
                    failed: f,
                    total,
                }
            };

            emit_event(
                &app,
                job_event(
                    &job,
                    TransferStatus::Queued,
                    0,
                    job.total_bytes,
                    0,
                    None,
                    &batch_snapshot,
                ),
            );

            let job_id = job.id.clone();
            let handle = tokio::spawn(async move {
                let _permit = permit;
                if cancel.load(Ordering::Relaxed) {
                    return (
                        job_id,
                        Err(AppError::new(AppError::TRANSFER_CANCELLED)),
                    );
                }

                let batch_for_job = {
                    let (c, f) = *counters.lock().await;
                    BatchSummary {
                        completed: c,
                        failed: f,
                        total,
                    }
                };

                let result = run_single_job(&conn, &app, &job, &cancel, &batch_for_job).await;

                {
                    let mut guard = counters.lock().await;
                    match &result {
                        Ok(()) => guard.0 += 1,
                        Err(e) if !e.is_transfer_cancelled() => guard.1 += 1,
                        _ => {}
                    }
                }

                (job_id, result)
            });
            handles.push(handle);
        }

        let mut cancelled = false;
        for handle in handles {
            if let Ok((_, Err(err))) = handle.await {
                if err.is_transfer_cancelled() {
                    cancelled = true;
                }
            }
        }

        let (completed, failed) = *counters.lock().await;
        let final_batch = BatchSummary {
            completed,
            failed,
            total,
        };
        emit_event(
            &app,
            SftpTransferEvent {
                job_id: "batch-complete".to_string(),
                file_name: String::new(),
                kind: TransferKind::Upload,
                status: if cancelled || cancel.load(Ordering::Relaxed) {
                    TransferStatus::Cancelled
                } else if failed > 0 {
                    TransferStatus::Failed
                } else {
                    TransferStatus::Completed
                },
                bytes_done: 0,
                total_bytes: 0,
                speed_bps: 0,
                error: None,
                remote_path: String::new(),
                local_path: None,
                dest_path: None,
                is_dir: false,
                batch: final_batch,
            },
        );
    }
}

async fn run_single_job(
    conn: &Arc<SshConnection>,
    app: &Arc<AppHandle>,
    job: &InternalJob,
    cancel: &Arc<AtomicBool>,
    batch: &BatchSummary,
) -> Result<(), AppError> {
    let mut attempt = 0u32;
    loop {
        if cancel.load(Ordering::Relaxed) {
            let cancelled = AppError::new(AppError::TRANSFER_CANCELLED);
            emit_job_status(
                app,
                job,
                TransferStatus::Cancelled,
                0,
                job.total_bytes,
                0,
                Some(error_payload(&cancelled)),
                batch,
            );
            return Err(cancelled);
        }

        emit_job_status(
            app,
            job,
            TransferStatus::Running,
            0,
            job.total_bytes,
            0,
            None,
            batch,
        );

        let sftp = open_sftp(conn).await?;
        let progress_app = app.clone();
        let progress_job = job.clone();
        let progress_batch = batch.clone();
        let on_progress = move |done: u64, total: u64, speed: u64| {
            emit_job_status(
                &progress_app,
                &progress_job,
                TransferStatus::Running,
                done,
                total,
                speed,
                None,
                &progress_batch,
            );
        };

        let result = match job.kind {
            TransferKind::Upload => {
                let local = job.local_path.as_ref().ok_or(AppError::new("NO_LOCAL_PATH"))?;
                sftp_upload_stream(&sftp, local, &job.remote_path, cancel, on_progress).await
            }
            TransferKind::Download => {
                let local = job.local_path.as_ref().ok_or(AppError::new("NO_LOCAL_PATH"))?;
                sftp_download_stream(&sftp, &job.remote_path, local, cancel, on_progress).await
            }
            TransferKind::Move => {
                let dest = job.local_path.as_ref().ok_or(AppError::new("NO_DEST_PATH"))?;
                sftp_move_path(&sftp, &job.remote_path, dest).await
            }
            TransferKind::Delete => {
                if job.is_dir {
                    sftp_delete_recursive(&sftp, &job.remote_path).await
                } else {
                    sftp.remove_file(&job.remote_path).await.map_err(|e| {
                        AppError::with_details("SFTP_FILE_DELETE_FAILED", e.to_string())
                    })
                }
            }
        };

        match result {
            Ok(()) => {
                emit_job_status(
                    app,
                    job,
                    TransferStatus::Completed,
                    job.total_bytes,
                    job.total_bytes,
                    0,
                    None,
                    batch,
                );
                return Ok(());
            }
            Err(err) if err.is_transfer_cancelled() => {
                emit_job_status(
                    app,
                    job,
                    TransferStatus::Cancelled,
                    0,
                    job.total_bytes,
                    0,
                    Some(error_payload(&err)),
                    batch,
                );
                return Err(err);
            }
            Err(err) => {
                attempt += 1;
                if attempt >= MAX_RETRIES {
                    emit_job_status(
                        app,
                        job,
                        TransferStatus::Failed,
                        0,
                        job.total_bytes,
                        0,
                        Some(error_payload(&err)),
                        batch,
                    );
                    return Err(err);
                }
                sleep_backoff(attempt - 1).await;
            }
        }
    }
}

fn emit_job_status(
    app: &AppHandle,
    job: &InternalJob,
    status: TransferStatus,
    bytes_done: u64,
    total_bytes: u64,
    speed_bps: u64,
    error: Option<String>,
    batch: &BatchSummary,
) {
    emit_event(
        app,
        job_event(job, status, bytes_done, total_bytes, speed_bps, error, batch),
    );
}

pub fn build_upload_jobs(items: Vec<UploadItem>) -> Vec<InternalJob> {
    items
        .into_iter()
        .enumerate()
        .map(|(i, item)| {
            let size = std::fs::metadata(&item.local_path)
                .map(|m| m.len())
                .unwrap_or(0);
            InternalJob {
                id: format!("job-{}", i),
                kind: TransferKind::Upload,
                local_path: Some(item.local_path),
                remote_path: item.remote_path,
                total_bytes: size,
                is_dir: false,
            }
        })
        .collect()
}

pub fn build_move_jobs(items: Vec<MoveItem>) -> Vec<InternalJob> {
    items
        .into_iter()
        .enumerate()
        .map(|(i, item)| InternalJob {
            id: format!("job-{}", i),
            kind: TransferKind::Move,
            local_path: Some(item.dest),
            remote_path: item.src,
            total_bytes: 0,
            is_dir: false,
        })
        .collect()
}

pub fn build_delete_jobs(items: Vec<DeleteItem>) -> Vec<InternalJob> {
    items
        .into_iter()
        .enumerate()
        .map(|(i, item)| InternalJob {
            id: format!("job-{}", i),
            kind: TransferKind::Delete,
            local_path: None,
            remote_path: item.path,
            total_bytes: 0,
            is_dir: item.is_dir,
        })
        .collect()
}
