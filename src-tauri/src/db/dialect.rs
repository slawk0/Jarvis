//! Per-engine SQL identifier quoting and lightweight validation.
//!
//! Values are always passed to the database via parameter binding (never string
//! interpolation). Identifiers (table/column/database names) cannot be bound, so
//! they are quoted here and validated to reject control characters.

use crate::app_error::AppError;

/// Reject identifiers containing characters that have no place in a table or
/// column name and could be used to break out of quoting.
pub fn validate_ident(name: &str) -> Result<(), AppError> {
    if name.is_empty() || name.len() > 128 {
        return Err(AppError::new("INVALID_DB_IDENTIFIER"));
    }
    if name.chars().any(|c| c == '\0' || c == '\n' || c == '\r') {
        return Err(AppError::new("INVALID_DB_IDENTIFIER"));
    }
    Ok(())
}

/// Quote an identifier for the given engine, escaping the quote character.
pub fn quote_ident(engine: &str, name: &str) -> String {
    match engine {
        "mysql" => format!("`{}`", name.replace('`', "``")),
        _ => format!("\"{}\"", name.replace('"', "\"\"")),
    }
}

/// Positional bind placeholder for the given engine (1-based index for Postgres).
pub fn placeholder(engine: &str, idx: usize) -> String {
    match engine {
        "postgres" => format!("${}", idx),
        _ => "?".to_string(),
    }
}
