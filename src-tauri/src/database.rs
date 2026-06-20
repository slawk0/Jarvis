use crate::app_error::AppError;
use crate::du_size::shell_single_quote;
use crate::AppState;
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub message: Option<String>,
}

fn validate_engine(engine: &str) -> Result<(), AppError> {
    if matches!(engine, "mysql" | "postgres") {
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

fn validate_container(id: &str) -> Result<(), AppError> {
    if id.len() > 128 {
        return Err(AppError::new("INVALID_CONTAINER_ID"));
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
    {
        return Err(AppError::new("INVALID_CONTAINER_ID"));
    }
    Ok(())
}

/// Build the inner client command (without docker wrapping) for the engine.
fn build_inner(
    engine: &str,
    host: &str,
    port: &str,
    user: &str,
    password: &str,
    database: &Option<String>,
    sql: &str,
) -> String {
    let q_host = shell_single_quote(host);
    let q_port = shell_single_quote(port);
    let q_user = shell_single_quote(user);
    let q_sql = shell_single_quote(sql);

    if engine == "mysql" {
        let db_arg = database
            .as_ref()
            .filter(|d| !d.is_empty())
            .map(|d| format!(" -D {}", shell_single_quote(d)))
            .unwrap_or_default();
        // -p<pass> must have no space after -p
        format!(
            "mysql --batch --raw --default-character-set=utf8 -h {} -P {} -u {} -p{}{} -e {}",
            q_host,
            q_port,
            q_user,
            shell_single_quote(password),
            db_arg,
            q_sql
        )
    } else {
        let db = database
            .as_ref()
            .filter(|d| !d.is_empty())
            .cloned()
            .unwrap_or_else(|| "postgres".to_string());
        format!(
            "PGPASSWORD={} psql -h {} -p {} -U {} -d {} -A -F '\\t' --pset footer=off -c {}",
            shell_single_quote(password),
            q_host,
            q_port,
            q_user,
            shell_single_quote(&db),
            q_sql
        )
    }
}

fn parse_output(stdout: &str) -> QueryResult {
    let trimmed = stdout.trim_end_matches(['\n', '\r']);
    if trimmed.is_empty() {
        return QueryResult {
            columns: Vec::new(),
            rows: Vec::new(),
            message: Some("OK".to_string()),
        };
    }
    let lines: Vec<&str> = trimmed.split('\n').collect();
    // A single line without tabs is a status message (e.g. "UPDATE 1").
    if lines.len() == 1 && !lines[0].contains('\t') {
        return QueryResult {
            columns: Vec::new(),
            rows: Vec::new(),
            message: Some(lines[0].to_string()),
        };
    }
    let columns: Vec<String> = lines[0].split('\t').map(|s| s.to_string()).collect();
    let rows: Vec<Vec<String>> = lines[1..]
        .iter()
        .map(|line| line.split('\t').map(|s| s.to_string()).collect())
        .collect();
    QueryResult {
        columns,
        rows,
        message: None,
    }
}

#[tauri::command]
pub async fn db_query(
    state: State<'_, AppState>,
    engine: String,
    host: String,
    port: String,
    user: String,
    password: String,
    database: Option<String>,
    container: Option<String>,
    sql: String,
) -> Result<QueryResult, AppError> {
    validate_engine(&engine)?;
    validate_simple(&host, "INVALID_DB_HOST", 255)?;
    validate_simple(&port, "INVALID_DB_PORT", 11)?;
    if !port.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::new("INVALID_DB_PORT"));
    }
    validate_simple(&user, "INVALID_DB_USER", 128)?;
    if sql.is_empty() || sql.len() > 200_000 {
        return Err(AppError::new("INVALID_DB_SQL"));
    }
    if let Some(c) = &container {
        validate_container(c)?;
    }

    let conn = {
        let g = state.connection.lock();
        g.as_ref()
            .ok_or_else(|| AppError::new("NO_SSH_CONNECTION"))?
            .clone()
    };

    let inner = build_inner(&engine, &host, &port, &user, &password, &database, &sql);
    let cmd = match &container {
        Some(c) if !c.is_empty() => {
            format!("docker exec -i {} sh -c {}", shell_single_quote(c), shell_single_quote(&inner))
        }
        _ => inner,
    };

    let (exit_code, stdout, stderr) = conn.exec(&cmd).await?;
    if exit_code != 0 {
        // Filter the noisy mysql password warning so the real error shows.
        let cleaned: String = stderr
            .lines()
            .filter(|l| !l.contains("Using a password on the command line"))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(AppError::with_details(
            "DB_QUERY_FAILED",
            if cleaned.trim().is_empty() {
                stdout
            } else {
                cleaned
            },
        ));
    }
    Ok(parse_output(&stdout))
}
