pub fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub fn du_folder_cmd(dir_path: &str) -> String {
    let folder = format!("{}/", dir_path.trim_end_matches('/'));
    let quoted = shell_single_quote(&folder);
    format!(
        r#"du -sk {} | awk '{{printf "%.2f MB\t%s\n", $1/1024, $2}}'"#,
        quoted
    )
}

pub fn parse_du_mb_line(line: &str) -> Result<u64, String> {
    let line = line.trim();
    if line.is_empty() {
        return Err("Pusty wynik du".to_string());
    }

    let mut parts = line.split_whitespace();
    let mb_str = parts
        .next()
        .ok_or_else(|| format!("Nieprawidłowy format du: {}", line))?;
    let unit = parts
        .next()
        .ok_or_else(|| format!("Nieprawidłowy format du: {}", line))?;

    if unit != "MB" {
        return Err(format!("Nieprawidłowy format du: {}", line));
    }

    let mb: f64 = mb_str
        .parse()
        .map_err(|_| format!("Nie można sparsować rozmiaru: {}", line))?;

    Ok((mb * 1024.0 * 1024.0).round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_du_command() {
        let cmd = du_folder_cmd("/var/log");
        assert!(cmd.contains("du -sk '/var/log/'"));
        assert!(cmd.contains("awk"));
    }

    #[test]
    fn parses_mb_line() {
        let bytes = parse_du_mb_line("12.34 MB\t/var/log").unwrap();
        assert_eq!(bytes, (12.34_f64 * 1024.0 * 1024.0).round() as u64);
    }
}
