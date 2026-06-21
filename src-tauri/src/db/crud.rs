//! Parameterized row writes (INSERT / UPDATE / DELETE).
//!
//! All values are bound, never interpolated. PostgreSQL is strict about bind
//! parameter types, so each placeholder is cast to the column's `udt_name`
//! (`$1::int4`, `$1::timestamptz`, ...). MySQL coerces string binds to the
//! target column type, so no casts are needed there.

use super::dialect::{placeholder, quote_ident, validate_ident};
use super::introspect::fetch;
use super::{get_conn, map_db_err, DbExecutor, QueryResult};
use crate::app_error::AppError;
use crate::AppState;
use serde::Deserialize;
use std::collections::HashMap;
use tauri::State;

#[derive(Deserialize)]
pub struct CellValue {
    pub column: String,
    pub value: Option<String>,
}

/// Map of column name -> Postgres `udt_name`, used to cast bind parameters.
async fn pg_types(executor: &DbExecutor, table: &str) -> Result<HashMap<String, String>, AppError> {
    let sql = "SELECT column_name, udt_name FROM information_schema.columns \
               WHERE table_schema = 'public' AND table_name = $1";
    let res = fetch(executor, sql, &[Some(table.to_string())]).await?;
    let mut map = HashMap::new();
    for row in &res.rows {
        if let (Some(Some(name)), Some(Some(udt))) = (row.first(), row.get(1)) {
            map.insert(name.clone(), udt.clone());
        }
    }
    Ok(map)
}

/// A placeholder with the optional Postgres type cast applied.
fn cast_placeholder(engine: &str, idx: usize, types: &HashMap<String, String>, col: &str) -> String {
    let ph = placeholder(engine, idx);
    if engine == "postgres" {
        if let Some(t) = types.get(col) {
            return format!("{}::{}", ph, t);
        }
    }
    ph
}

fn qual_table(engine: &str, database: Option<&str>, table: &str) -> String {
    match engine {
        "mysql" => match database.filter(|d| !d.is_empty()) {
            Some(db) => format!("{}.{}", quote_ident("mysql", db), quote_ident("mysql", table)),
            None => quote_ident("mysql", table),
        },
        _ => quote_ident("postgres", table),
    }
}

async fn execute(
    executor: &DbExecutor,
    sql: &str,
    binds: &[Option<String>],
) -> Result<u64, AppError> {
    match executor {
        DbExecutor::MySql(pool) => {
            let mut q = sqlx::query(sql);
            for b in binds {
                q = q.bind(b.clone());
            }
            Ok(q.execute(pool).await.map_err(map_db_err)?.rows_affected())
        }
        DbExecutor::Postgres(pool) => {
            let mut q = sqlx::query(sql);
            for b in binds {
                q = q.bind(b.clone());
            }
            Ok(q.execute(pool).await.map_err(map_db_err)?.rows_affected())
        }
        DbExecutor::Sqlite { .. } => Err(AppError::new("DB_SQLITE_UNSUPPORTED")),
    }
}

#[tauri::command]
pub async fn db_insert_row(
    state: State<'_, AppState>,
    connection_id: String,
    database: Option<String>,
    table: String,
    values: Vec<CellValue>,
) -> Result<QueryResult, AppError> {
    let (executor, engine, _) = get_conn(&state, &connection_id)?;
    validate_ident(&table)?;
    if values.is_empty() {
        return Err(AppError::new("DB_NO_VALUES"));
    }
    let types = if engine == "postgres" {
        pg_types(&executor, &table).await?
    } else {
        HashMap::new()
    };

    let mut cols = Vec::new();
    let mut phs = Vec::new();
    let mut binds = Vec::new();
    for (i, cell) in values.iter().enumerate() {
        validate_ident(&cell.column)?;
        cols.push(quote_ident(&engine, &cell.column));
        phs.push(cast_placeholder(&engine, i + 1, &types, &cell.column));
        binds.push(cell.value.clone());
    }
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        qual_table(&engine, database.as_deref(), &table),
        cols.join(", "),
        phs.join(", ")
    );
    let affected = execute(&executor, &sql, &binds).await?;
    Ok(QueryResult {
        rows_affected: Some(affected),
        message: Some(format!("{} row(s) inserted", affected)),
        ..Default::default()
    })
}

#[tauri::command]
pub async fn db_update_row(
    state: State<'_, AppState>,
    connection_id: String,
    database: Option<String>,
    table: String,
    values: Vec<CellValue>,
    pk: Vec<CellValue>,
) -> Result<QueryResult, AppError> {
    let (executor, engine, _) = get_conn(&state, &connection_id)?;
    validate_ident(&table)?;
    if values.is_empty() || pk.is_empty() {
        return Err(AppError::new("DB_NO_VALUES"));
    }
    let types = if engine == "postgres" {
        pg_types(&executor, &table).await?
    } else {
        HashMap::new()
    };

    let mut set_parts = Vec::new();
    let mut binds = Vec::new();
    let mut idx = 1;
    for cell in &values {
        validate_ident(&cell.column)?;
        set_parts.push(format!(
            "{} = {}",
            quote_ident(&engine, &cell.column),
            cast_placeholder(&engine, idx, &types, &cell.column)
        ));
        binds.push(cell.value.clone());
        idx += 1;
    }
    let mut where_parts = Vec::new();
    for cell in &pk {
        validate_ident(&cell.column)?;
        where_parts.push(format!(
            "{} = {}",
            quote_ident(&engine, &cell.column),
            cast_placeholder(&engine, idx, &types, &cell.column)
        ));
        binds.push(cell.value.clone());
        idx += 1;
    }
    let sql = format!(
        "UPDATE {} SET {} WHERE {}",
        qual_table(&engine, database.as_deref(), &table),
        set_parts.join(", "),
        where_parts.join(" AND ")
    );
    let affected = execute(&executor, &sql, &binds).await?;
    Ok(QueryResult {
        rows_affected: Some(affected),
        message: Some(format!("{} row(s) updated", affected)),
        ..Default::default()
    })
}

#[tauri::command]
pub async fn db_delete_rows(
    state: State<'_, AppState>,
    connection_id: String,
    database: Option<String>,
    table: String,
    rows: Vec<Vec<CellValue>>,
) -> Result<QueryResult, AppError> {
    let (executor, engine, _) = get_conn(&state, &connection_id)?;
    validate_ident(&table)?;
    if rows.is_empty() {
        return Err(AppError::new("DB_NO_VALUES"));
    }
    let types = if engine == "postgres" {
        pg_types(&executor, &table).await?
    } else {
        HashMap::new()
    };
    let tref = qual_table(&engine, database.as_deref(), &table);

    let mut total = 0u64;
    for row in &rows {
        if row.is_empty() {
            continue;
        }
        let mut where_parts = Vec::new();
        let mut binds = Vec::new();
        for (i, cell) in row.iter().enumerate() {
            validate_ident(&cell.column)?;
            where_parts.push(format!(
                "{} = {}",
                quote_ident(&engine, &cell.column),
                cast_placeholder(&engine, i + 1, &types, &cell.column)
            ));
            binds.push(cell.value.clone());
        }
        let sql = format!("DELETE FROM {} WHERE {}", tref, where_parts.join(" AND "));
        total += execute(&executor, &sql, &binds).await?;
    }
    Ok(QueryResult {
        rows_affected: Some(total),
        message: Some(format!("{} row(s) deleted", total)),
        ..Default::default()
    })
}
