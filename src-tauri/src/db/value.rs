//! Generic decoding of arbitrary SQL column values into displayable strings.
//!
//! sqlx is statically typed, so to build a generic database browser we probe
//! each cell against a cascade of Rust types and take the first that decodes.
//! `try_get` is type-checked by sqlx and returns `Err` (never mis-decodes) when
//! the requested type is incompatible, which makes the cascade safe.
//!
//! A SQL `NULL` is represented as `None` so the frontend can render it
//! distinctly from an empty string.

use sqlx::{Column, Row, ValueRef};

/// Represent raw bytes: valid UTF-8 as text, otherwise a hex literal.
fn bytes_repr(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => {
            let mut out = String::with_capacity(2 + bytes.len() * 2);
            out.push_str("0x");
            for b in bytes {
                out.push_str(&format!("{:02x}", b));
            }
            out
        }
    }
}

pub fn mysql_row_to_strings(row: &sqlx::mysql::MySqlRow) -> Vec<Option<String>> {
    (0..row.columns().len())
        .map(|i| mysql_value(row, i))
        .collect()
}

fn mysql_value(row: &sqlx::mysql::MySqlRow, i: usize) -> Option<String> {
    match row.try_get_raw(i) {
        Ok(raw) if raw.is_null() => return None,
        Ok(_) => {}
        Err(_) => return None,
    }
    if let Ok(v) = row.try_get::<String, _>(i) {
        return Some(v);
    }
    if let Ok(v) = row.try_get::<bool, _>(i) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<i64, _>(i) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<u64, _>(i) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<f64, _>(i) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<rust_decimal::Decimal, _>(i) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<chrono::NaiveDateTime, _>(i) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<chrono::NaiveDate, _>(i) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<chrono::NaiveTime, _>(i) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<serde_json::Value, _>(i) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<Vec<u8>, _>(i) {
        return Some(bytes_repr(&v));
    }
    Some(String::new())
}

pub fn pg_row_to_strings(row: &sqlx::postgres::PgRow) -> Vec<Option<String>> {
    (0..row.columns().len())
        .map(|i| pg_value(row, i))
        .collect()
}

fn pg_value(row: &sqlx::postgres::PgRow, i: usize) -> Option<String> {
    match row.try_get_raw(i) {
        Ok(raw) if raw.is_null() => return None,
        Ok(_) => {}
        Err(_) => return None,
    }
    if let Ok(v) = row.try_get::<String, _>(i) {
        return Some(v);
    }
    if let Ok(v) = row.try_get::<bool, _>(i) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<i16, _>(i) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<i32, _>(i) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<i64, _>(i) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<f32, _>(i) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<f64, _>(i) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<rust_decimal::Decimal, _>(i) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<chrono::DateTime<chrono::Utc>, _>(i) {
        return Some(v.to_rfc3339());
    }
    if let Ok(v) = row.try_get::<chrono::NaiveDateTime, _>(i) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<chrono::NaiveDate, _>(i) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<chrono::NaiveTime, _>(i) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<serde_json::Value, _>(i) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<uuid::Uuid, _>(i) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<Vec<u8>, _>(i) {
        return Some(bytes_repr(&v));
    }
    Some(String::new())
}

pub fn mysql_columns(row: &sqlx::mysql::MySqlRow) -> Vec<String> {
    row.columns().iter().map(|c| c.name().to_string()).collect()
}

pub fn pg_columns(row: &sqlx::postgres::PgRow) -> Vec<String> {
    row.columns().iter().map(|c| c.name().to_string()).collect()
}
