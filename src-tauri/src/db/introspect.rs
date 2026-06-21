//! Schema introspection and paginated row browsing.

use super::dialect::{placeholder, quote_ident, validate_ident};
use super::{get_conn, map_db_err, value, DbExecutor, QueryResult};
use crate::app_error::AppError;
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

// ---------------------------------------------------------------------------
// Low-level fetch helpers (parameterized, no DB switching — tables are qualified)
// ---------------------------------------------------------------------------

pub async fn fetch(
    executor: &DbExecutor,
    sql: &str,
    binds: &[Option<String>],
) -> Result<QueryResult, AppError> {
    match executor {
        DbExecutor::MySql(pool) => {
            let mut q = sqlx::query(sql);
            for b in binds {
                q = q.bind(b.clone());
            }
            let rows = q.fetch_all(pool).await.map_err(map_db_err)?;
            let mut res = QueryResult::default();
            if let Some(first) = rows.first() {
                res.columns = value::mysql_columns(first);
            }
            for row in &rows {
                res.rows.push(value::mysql_row_to_strings(row));
            }
            Ok(res)
        }
        DbExecutor::Postgres(pool) => {
            let mut q = sqlx::query(sql);
            for b in binds {
                q = q.bind(b.clone());
            }
            let rows = q.fetch_all(pool).await.map_err(map_db_err)?;
            let mut res = QueryResult::default();
            if let Some(first) = rows.first() {
                res.columns = value::pg_columns(first);
            }
            for row in &rows {
                res.rows.push(value::pg_row_to_strings(row));
            }
            Ok(res)
        }
        DbExecutor::Sqlite { .. } => Err(AppError::new("DB_SQLITE_UNSUPPORTED")),
    }
}

/// Fully-qualified table reference for the engine.
fn qual_table(engine: &str, database: Option<&str>, table: &str) -> String {
    match engine {
        "mysql" => match database.filter(|d| !d.is_empty()) {
            Some(db) => format!("{}.{}", quote_ident("mysql", db), quote_ident("mysql", table)),
            None => quote_ident("mysql", table),
        },
        // Postgres connection is bound to a database; default to the public schema.
        _ => quote_ident("postgres", table),
    }
}

// ---------------------------------------------------------------------------
// db_list_databases
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn db_list_databases(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<Vec<String>, AppError> {
    let (executor, engine, _) = get_conn(&state, &connection_id)?;
    let sql = match engine.as_str() {
        "mysql" => "SHOW DATABASES".to_string(),
        _ => "SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname"
            .to_string(),
    };
    let res = fetch(&executor, &sql, &[]).await?;
    Ok(res
        .rows
        .into_iter()
        .filter_map(|mut r| r.drain(..).next().flatten())
        .collect())
}

// ---------------------------------------------------------------------------
// db_list_tables
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct TableInfo {
    pub name: String,
    /// "table" or "view".
    pub kind: String,
}

#[tauri::command]
pub async fn db_list_tables(
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
) -> Result<Vec<TableInfo>, AppError> {
    let (executor, engine, _) = get_conn(&state, &connection_id)?;
    let res = match engine.as_str() {
        "mysql" => {
            validate_ident(&database)?;
            let sql = format!("SHOW FULL TABLES FROM {}", quote_ident("mysql", &database));
            fetch(&executor, &sql, &[]).await?
        }
        _ => {
            let sql = "SELECT table_name, table_type FROM information_schema.tables \
                       WHERE table_schema = 'public' ORDER BY table_name";
            fetch(&executor, sql, &[]).await?
        }
    };
    let mut out = Vec::new();
    for row in res.rows {
        let name = row.first().and_then(|c| c.clone()).unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let type_raw = row.get(1).and_then(|c| c.clone()).unwrap_or_default();
        let kind = if type_raw.to_uppercase().contains("VIEW") {
            "view"
        } else {
            "table"
        };
        out.push(TableInfo {
            name,
            kind: kind.to_string(),
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// db_table_structure
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub default: Option<String>,
    pub key: String,
    pub extra: String,
    pub comment: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    pub primary: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignKeyInfo {
    pub name: String,
    pub column: String,
    pub ref_table: String,
    pub ref_column: String,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TableStructure {
    pub columns: Vec<ColumnInfo>,
    pub indexes: Vec<IndexInfo>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
    /// Primary-key column names — used by the frontend to build safe row WHEREs.
    pub primary_key: Vec<String>,
}

#[tauri::command]
pub async fn db_table_structure(
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    table: String,
) -> Result<TableStructure, AppError> {
    let (executor, engine, _) = get_conn(&state, &connection_id)?;
    validate_ident(&table)?;
    if engine == "mysql" {
        mysql_structure(&executor, &database, &table).await
    } else {
        pg_structure(&executor, &table).await
    }
}

async fn mysql_structure(
    executor: &DbExecutor,
    database: &str,
    table: &str,
) -> Result<TableStructure, AppError> {
    validate_ident(database)?;
    let tref = format!(
        "{}.{}",
        quote_ident("mysql", database),
        quote_ident("mysql", table)
    );
    let mut st = TableStructure::default();

    // Columns: SHOW FULL COLUMNS -> Field, Type, Collation, Null, Key, Default, Extra, Privileges, Comment
    let cols = fetch(executor, &format!("SHOW FULL COLUMNS FROM {}", tref), &[]).await?;
    for row in &cols.rows {
        let get = |i: usize| row.get(i).and_then(|c| c.clone());
        let name = get(0).unwrap_or_default();
        let key = get(4).unwrap_or_default();
        if key == "PRI" {
            st.primary_key.push(name.clone());
        }
        st.columns.push(ColumnInfo {
            name,
            data_type: get(1).unwrap_or_default(),
            nullable: get(3).map(|v| v == "YES").unwrap_or(false),
            default: get(5),
            key,
            extra: get(6).unwrap_or_default(),
            comment: get(8).unwrap_or_default(),
        });
    }

    // Indexes: SHOW INDEX -> Table, Non_unique, Key_name, Seq_in_index, Column_name, ...
    let idx = fetch(executor, &format!("SHOW INDEX FROM {}", tref), &[]).await?;
    use std::collections::BTreeMap;
    let mut grouped: BTreeMap<String, IndexInfo> = BTreeMap::new();
    for row in &idx.rows {
        let get = |i: usize| row.get(i).and_then(|c| c.clone()).unwrap_or_default();
        let key_name = get(2);
        let non_unique = get(1) == "1";
        let col = get(4);
        let entry = grouped.entry(key_name.clone()).or_insert_with(|| IndexInfo {
            name: key_name.clone(),
            columns: Vec::new(),
            unique: !non_unique,
            primary: key_name == "PRIMARY",
        });
        entry.columns.push(col);
    }
    st.indexes = grouped.into_values().collect();

    // Foreign keys
    let fk_sql = "SELECT CONSTRAINT_NAME, COLUMN_NAME, REFERENCED_TABLE_NAME, REFERENCED_COLUMN_NAME \
                  FROM information_schema.KEY_COLUMN_USAGE \
                  WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? AND REFERENCED_TABLE_NAME IS NOT NULL";
    let fks = fetch(
        executor,
        fk_sql,
        &[Some(database.to_string()), Some(table.to_string())],
    )
    .await?;
    for row in &fks.rows {
        let get = |i: usize| row.get(i).and_then(|c| c.clone()).unwrap_or_default();
        st.foreign_keys.push(ForeignKeyInfo {
            name: get(0),
            column: get(1),
            ref_table: get(2),
            ref_column: get(3),
        });
    }
    Ok(st)
}

async fn pg_structure(executor: &DbExecutor, table: &str) -> Result<TableStructure, AppError> {
    let mut st = TableStructure::default();

    // Columns
    let col_sql = "SELECT column_name, \
                   CASE WHEN character_maximum_length IS NOT NULL \
                        THEN udt_name || '(' || character_maximum_length || ')' ELSE udt_name END, \
                   is_nullable, column_default \
                   FROM information_schema.columns \
                   WHERE table_schema = 'public' AND table_name = $1 ORDER BY ordinal_position";
    let cols = fetch(executor, col_sql, &[Some(table.to_string())]).await?;
    for row in &cols.rows {
        let get = |i: usize| row.get(i).and_then(|c| c.clone());
        st.columns.push(ColumnInfo {
            name: get(0).unwrap_or_default(),
            data_type: get(1).unwrap_or_default(),
            nullable: get(2).map(|v| v == "YES").unwrap_or(false),
            default: get(3),
            key: String::new(),
            extra: String::new(),
            comment: String::new(),
        });
    }

    // Primary key columns
    let pk_sql = "SELECT a.attname FROM pg_index i \
                  JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey) \
                  WHERE i.indrelid = ($1)::regclass AND i.indisprimary";
    if let Ok(pk) = fetch(executor, pk_sql, &[Some(quote_ident("postgres", table))]).await {
        for row in &pk.rows {
            if let Some(Some(name)) = row.first() {
                st.primary_key.push(name.clone());
                if let Some(c) = st.columns.iter_mut().find(|c| &c.name == name) {
                    c.key = "PRI".to_string();
                }
            }
        }
    }

    // Indexes
    let idx_sql = "SELECT indexname, indexdef FROM pg_indexes \
                   WHERE schemaname = 'public' AND tablename = $1";
    if let Ok(idx) = fetch(executor, idx_sql, &[Some(table.to_string())]).await {
        for row in &idx.rows {
            let name = row.first().and_then(|c| c.clone()).unwrap_or_default();
            let def = row.get(1).and_then(|c| c.clone()).unwrap_or_default();
            let unique = def.to_uppercase().contains("UNIQUE");
            // Extract columns between the outermost parentheses.
            let columns = def
                .split_once('(')
                .and_then(|(_, rest)| rest.rsplit_once(')').map(|(inner, _)| inner))
                .map(|inner| inner.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default();
            st.indexes.push(IndexInfo {
                name: name.clone(),
                columns,
                unique,
                primary: st.primary_key.iter().any(|_| name.ends_with("_pkey")),
            });
        }
    }

    // Foreign keys
    let fk_sql = "SELECT tc.constraint_name, kcu.column_name, ccu.table_name, ccu.column_name \
                  FROM information_schema.table_constraints tc \
                  JOIN information_schema.key_column_usage kcu ON tc.constraint_name = kcu.constraint_name \
                  JOIN information_schema.constraint_column_usage ccu ON ccu.constraint_name = tc.constraint_name \
                  WHERE tc.constraint_type = 'FOREIGN KEY' AND tc.table_schema = 'public' AND tc.table_name = $1";
    if let Ok(fks) = fetch(executor, fk_sql, &[Some(table.to_string())]).await {
        for row in &fks.rows {
            let get = |i: usize| row.get(i).and_then(|c| c.clone()).unwrap_or_default();
            st.foreign_keys.push(ForeignKeyInfo {
                name: get(0),
                column: get(1),
                ref_table: get(2),
                ref_column: get(3),
            });
        }
    }
    Ok(st)
}

// ---------------------------------------------------------------------------
// db_select — paginated, sorted, filtered browse
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct Filter {
    pub column: String,
    pub op: String,
    pub value: Option<String>,
}

#[derive(Serialize)]
pub struct SelectResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
    pub total: i64,
}

fn build_where(
    engine: &str,
    filters: &[Filter],
    start_idx: usize,
) -> Result<(String, Vec<Option<String>>), AppError> {
    if filters.is_empty() {
        return Ok((String::new(), Vec::new()));
    }
    let cast = if engine == "mysql" { "CHAR" } else { "TEXT" };
    let mut clauses = Vec::new();
    let mut binds = Vec::new();
    let mut idx = start_idx;
    for f in filters {
        validate_ident(&f.column)?;
        let op = match f.op.as_str() {
            "=" | "!=" | "<" | ">" | "<=" | ">=" | "LIKE" => f.op.as_str(),
            _ => "=",
        };
        let ph = placeholder(engine, idx);
        idx += 1;
        clauses.push(format!(
            "CAST({} AS {}) {} {}",
            quote_ident(engine, &f.column),
            cast,
            op,
            ph
        ));
        binds.push(f.value.clone());
    }
    Ok((format!(" WHERE {}", clauses.join(" AND ")), binds))
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn db_select(
    state: State<'_, AppState>,
    connection_id: String,
    database: Option<String>,
    table: String,
    filters: Vec<Filter>,
    order_by: Option<String>,
    order_dir: Option<String>,
    limit: i64,
    offset: i64,
) -> Result<SelectResult, AppError> {
    let (executor, engine, _) = get_conn(&state, &connection_id)?;
    validate_ident(&table)?;
    let tref = qual_table(&engine, database.as_deref(), &table);

    let (where_sql, binds) = build_where(&engine, &filters, 1)?;

    let mut order_sql = String::new();
    if let Some(ob) = order_by.as_ref().filter(|s| !s.is_empty()) {
        validate_ident(ob)?;
        let dir = match order_dir.as_deref() {
            Some("desc") | Some("DESC") => "DESC",
            _ => "ASC",
        };
        order_sql = format!(" ORDER BY {} {}", quote_ident(&engine, ob), dir);
    }

    let limit = limit.clamp(1, 1000);
    let offset = offset.max(0);

    let data_sql = format!(
        "SELECT * FROM {}{}{} LIMIT {} OFFSET {}",
        tref, where_sql, order_sql, limit, offset
    );
    let data = fetch(&executor, &data_sql, &binds).await?;

    let count_sql = format!("SELECT COUNT(*) FROM {}{}", tref, where_sql);
    let count = fetch(&executor, &count_sql, &binds).await?;
    let total = count
        .rows
        .first()
        .and_then(|r| r.first())
        .and_then(|c| c.clone())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    Ok(SelectResult {
        columns: data.columns,
        rows: data.rows,
        total,
    })
}
