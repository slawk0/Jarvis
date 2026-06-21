//! Database management ("Adminer"-like) backend.
//!
//! MySQL and PostgreSQL are accessed through the native `sqlx` driver, tunneled
//! over the active SSH connection (see [`crate::ssh_tunnel`]). This yields typed
//! results, correct `NULL`/binary handling and safe parameter binding — none of
//! which the previous `mysql`/`psql` CLI text-parsing approach could provide.
//!
//! A connection is established once via [`db_connect`] (which returns a
//! `connection_id`); subsequent commands reference it by id.

pub mod crud;
pub mod dialect;
pub mod introspect;
pub mod value;

use crate::app_error::AppError;
use crate::ssh_tunnel::{start_tunnel, TunnelHandle};
use crate::AppState;
use futures::TryStreamExt;
use serde::Serialize;
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Either, MySqlPool, PgPool};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::State;

/// Result of a query: column names plus rows of nullable string cells, or a
/// status message / affected-row count for non-SELECT statements.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
    pub message: Option<String>,
    pub rows_affected: Option<u64>,
}

/// Backend executor behind a logical connection. Pools clone cheaply (they are
/// `Arc` internally), so they can be taken out of the connection map without
/// holding the lock across `await`.
#[derive(Clone)]
pub enum DbExecutor {
    MySql(MySqlPool),
    Postgres(PgPool),
    /// SQLite is driven through the `sqlite3` CLI over SSH (the native driver
    /// needs local file access). `path` is the remote database file.
    Sqlite { path: String },
}

pub struct DbConnection {
    pub executor: DbExecutor,
    /// Kept alive for the lifetime of the connection; dropped on disconnect.
    pub _tunnel: Option<TunnelHandle>,
    pub engine: String,
    /// For PostgreSQL the connection is bound to one database.
    pub database: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectResult {
    pub connection_id: String,
}

static CONN_SEQ: AtomicU64 = AtomicU64::new(1);

fn new_conn_id() -> String {
    let n = CONN_SEQ.fetch_add(1, Ordering::Relaxed);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("db-{:x}-{}", t, n)
}

pub fn map_db_err(e: sqlx::Error) -> AppError {
    AppError::with_details("DB_QUERY_FAILED", e.to_string())
}

fn validate_engine(engine: &str) -> Result<(), AppError> {
    if matches!(engine, "mysql" | "postgres" | "sqlite") {
        Ok(())
    } else {
        Err(AppError::new("INVALID_DB_ENGINE"))
    }
}

fn validate_simple(value: &str, code: &'static str, max: usize) -> Result<(), AppError> {
    if value.is_empty() || value.len() > max {
        return Err(AppError::new(code));
    }
    if value.contains('\n') || value.contains('\r') || value.contains('\0') {
        return Err(AppError::new(code));
    }
    Ok(())
}

/// Resolve the tunnel target for a Docker-hosted database: the container's IP on
/// its Docker network, reachable from the remote host even without a published
/// port. Falls back to the supplied host if inspection yields nothing.
async fn resolve_docker_target(
    conn: &crate::ssh::SshConnection,
    container: &str,
    fallback_host: &str,
) -> String {
    let q = crate::du_size::shell_single_quote(container);
    let cmd = format!(
        "docker inspect -f '{{{{range .NetworkSettings.Networks}}}}{{{{.IPAddress}}}} {{{{end}}}}' {}",
        q
    );
    if let Ok((0, stdout, _)) = conn.exec(&cmd).await {
        if let Some(ip) = stdout.split_whitespace().next() {
            if !ip.is_empty() {
                return ip.to_string();
            }
        }
    }
    fallback_host.to_string()
}

#[tauri::command]
pub async fn db_connect(
    state: State<'_, AppState>,
    engine: String,
    host: String,
    port: String,
    user: String,
    password: String,
    database: Option<String>,
    container: Option<String>,
) -> Result<ConnectResult, AppError> {
    validate_engine(&engine)?;
    validate_simple(&host, "INVALID_DB_HOST", 255)?;
    validate_simple(&port, "INVALID_DB_PORT", 11)?;
    let port_num: u16 = port
        .parse()
        .map_err(|_| AppError::new("INVALID_DB_PORT"))?;
    validate_simple(&user, "INVALID_DB_USER", 128)?;

    let conn = {
        let g = state.connection.lock();
        g.as_ref()
            .ok_or_else(|| AppError::new("NO_SSH_CONNECTION"))?
            .clone()
    };

    // Determine the host the tunnel forwards to on the remote side.
    let target_host = match &container {
        Some(c) if !c.is_empty() => resolve_docker_target(&conn, c, &host).await,
        _ => host.clone(),
    };

    let tunnel = start_tunnel(conn, target_host, port_num as u32).await?;
    let local_port = tunnel.local_addr.port();

    let db_clean = database.as_ref().filter(|d| !d.is_empty()).cloned();

    let executor = match engine.as_str() {
        "mysql" => {
            let opts = MySqlConnectOptions::new()
                .host("127.0.0.1")
                .port(local_port)
                .username(&user)
                .password(&password);
            let pool = MySqlPoolOptions::new()
                .max_connections(3)
                .connect_with(opts)
                .await
                .map_err(|e| AppError::with_details("DB_CONNECT_FAILED", e.to_string()))?;
            DbExecutor::MySql(pool)
        }
        "postgres" => {
            let dbname = db_clean.clone().unwrap_or_else(|| "postgres".to_string());
            let opts = PgConnectOptions::new()
                .host("127.0.0.1")
                .port(local_port)
                .username(&user)
                .password(&password)
                .database(&dbname);
            let pool = PgPoolOptions::new()
                .max_connections(3)
                .connect_with(opts)
                .await
                .map_err(|e| AppError::with_details("DB_CONNECT_FAILED", e.to_string()))?;
            DbExecutor::Postgres(pool)
        }
        _ => return Err(AppError::new("INVALID_DB_ENGINE")),
    };

    let id = new_conn_id();
    state.db_connections.lock().insert(
        id.clone(),
        DbConnection {
            executor,
            _tunnel: Some(tunnel),
            engine: engine.clone(),
            database: db_clean,
        },
    );

    Ok(ConnectResult { connection_id: id })
}

#[tauri::command]
pub async fn db_disconnect(state: State<'_, AppState>, connection_id: String) -> Result<(), AppError> {
    // Removing from the map drops the pool and the tunnel handle.
    state.db_connections.lock().remove(&connection_id);
    Ok(())
}

/// Clone the executor (and engine/default-db) out of the connection map without
/// holding the lock across `await`.
pub fn get_conn(state: &AppState, id: &str) -> Result<(DbExecutor, String, Option<String>), AppError> {
    let map = state.db_connections.lock();
    let c = map
        .get(id)
        .ok_or_else(|| AppError::new("DB_NOT_CONNECTED"))?;
    Ok((c.executor.clone(), c.engine.clone(), c.database.clone()))
}

/// Run arbitrary SQL (possibly multiple statements) and collect the last result
/// set plus the total affected rows. Used by the SQL editor and internally.
pub async fn run_sql(
    executor: &DbExecutor,
    database: Option<&str>,
    sql: &str,
) -> Result<QueryResult, AppError> {
    if sql.is_empty() || sql.len() > 2_000_000 {
        return Err(AppError::new("INVALID_DB_SQL"));
    }
    match executor {
        DbExecutor::MySql(pool) => {
            let mut conn = pool.acquire().await.map_err(map_db_err)?;
            if let Some(db) = database.filter(|d| !d.is_empty()) {
                let use_stmt = format!("USE {}", dialect::quote_ident("mysql", db));
                sqlx::query(&use_stmt)
                    .execute(&mut *conn)
                    .await
                    .map_err(map_db_err)?;
            }
            let mut result = QueryResult::default();
            let mut affected = 0u64;
            let mut stream = sqlx::raw_sql(sql).fetch_many(&mut *conn);
            while let Some(item) = stream.try_next().await.map_err(map_db_err)? {
                match item {
                    Either::Left(res) => affected += res.rows_affected(),
                    Either::Right(row) => {
                        if result.columns.is_empty() {
                            result.columns = value::mysql_columns(&row);
                        }
                        result.rows.push(value::mysql_row_to_strings(&row));
                    }
                }
            }
            finalize(result, affected)
        }
        DbExecutor::Postgres(pool) => {
            let mut result = QueryResult::default();
            let mut affected = 0u64;
            let mut stream = sqlx::raw_sql(sql).fetch_many(pool);
            while let Some(item) = stream.try_next().await.map_err(map_db_err)? {
                match item {
                    Either::Left(res) => affected += res.rows_affected(),
                    Either::Right(row) => {
                        if result.columns.is_empty() {
                            result.columns = value::pg_columns(&row);
                        }
                        result.rows.push(value::pg_row_to_strings(&row));
                    }
                }
            }
            finalize(result, affected)
        }
        DbExecutor::Sqlite { .. } => Err(AppError::new("DB_SQLITE_UNSUPPORTED")),
    }
}

fn finalize(mut result: QueryResult, affected: u64) -> Result<QueryResult, AppError> {
    if result.columns.is_empty() {
        result.rows_affected = Some(affected);
        result.message = Some(format!("OK, {} row(s) affected", affected));
    }
    Ok(result)
}

/// SQL editor command: run arbitrary SQL against a connection.
#[tauri::command]
pub async fn db_query(
    state: State<'_, AppState>,
    connection_id: String,
    database: Option<String>,
    sql: String,
) -> Result<QueryResult, AppError> {
    let (executor, _engine, default_db) = get_conn(&state, &connection_id)?;
    let db = database.or(default_db);
    run_sql(&executor, db.as_deref(), &sql).await
}

/// DDL / administrative statement (CREATE/ALTER/DROP/GRANT...). Same execution
/// path as `db_query` but named separately for clarity at call sites.
#[tauri::command]
pub async fn db_exec_sql(
    state: State<'_, AppState>,
    connection_id: String,
    database: Option<String>,
    sql: String,
) -> Result<QueryResult, AppError> {
    let (executor, _engine, default_db) = get_conn(&state, &connection_id)?;
    let db = database.or(default_db);
    run_sql(&executor, db.as_deref(), &sql).await
}
