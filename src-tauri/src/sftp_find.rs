use crate::du_size::shell_single_quote;
use crate::ssh::FileInfo;

pub fn normalize_find_root(root: &str) -> String {
    let trimmed = root.trim();
    if trimmed.is_empty() || trimmed.chars().all(|c| c == '/') {
        return "/".to_string();
    }
    let without_trailing = trimmed.trim_end_matches('/');
    if without_trailing.is_empty() {
        return "/".to_string();
    }
    without_trailing.to_string()
}

pub fn find_cmd(root: &str, query: &str, hide_hidden: bool) -> String {
    let root_q = shell_single_quote(&normalize_find_root(root));
    let pattern = shell_single_quote(&glob_pattern(query));

    let hidden_filter = if hide_hidden {
        " ! -path '*/.*' ! -name '.*'"
    } else {
        ""
    };

    format!(
        r#"find {} -iname {}{} -printf '%p\t%s\t%T@\t%y\t%m\n' 2>/dev/null | head -n 1000"#,
        root_q, pattern, hidden_filter
    )
}

fn glob_pattern(query: &str) -> String {
    let mut out = String::from('*');
    for c in query.chars() {
        match c {
            '*' | '?' | '[' | ']' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out.push('*');
    out
}

pub fn parse_find_output(stdout: &str) -> Vec<FileInfo> {
    let mut results = Vec::new();

    for line in stdout.lines() {
        if let Some(info) = parse_find_line(line) {
            results.push(info);
        }
    }

    results.sort_by(|a, b| {
        if a.is_dir != b.is_dir {
            b.is_dir.cmp(&a.is_dir)
        } else {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        }
    });

    results
}

fn parse_find_line(line: &str) -> Option<FileInfo> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() < 5 {
        return None;
    }

    let full_path = parts[0].trim();
    if full_path.is_empty() {
        return None;
    }

    let size = parts[1].trim().parse::<u64>().unwrap_or(0);
    let modified = parts[2]
        .trim()
        .parse::<f64>()
        .map(|value| value as u64)
        .unwrap_or(0);
    let is_dir = parts[3].trim() == "d";
    let permissions = u32::from_str_radix(parts[4].trim(), 8).ok();

    let name = full_path
        .rsplit('/')
        .next()
        .filter(|part| !part.is_empty())
        .unwrap_or(full_path)
        .to_string();

    Some(FileInfo {
        name,
        is_dir,
        size,
        permissions,
        modified,
        path: Some(full_path.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_find_with_hidden_filter() {
        let cmd = find_cmd("/var/www", "log", true);
        assert!(cmd.contains("find '/var/www'"));
        assert!(cmd.contains("! -path '*/.*'"));
        assert!(cmd.contains("-iname '*log*'"));
    }

    #[test]
    fn root_path_stays_slash() {
        assert_eq!(normalize_find_root("/"), "/");
        assert_eq!(normalize_find_root("///"), "/");
        assert_eq!(normalize_find_root(""), "/");

        let cmd = find_cmd("/", "etc", true);
        assert!(cmd.contains("find '/'"));
        assert!(!cmd.contains("find ''"));
    }

    #[test]
    fn parses_find_line() {
        let info = parse_find_line("/var/log/syslog\t1024\t1710000000.0\tf\t644\n").unwrap();
        assert_eq!(info.name, "syslog");
        assert_eq!(info.path.as_deref(), Some("/var/log/syslog"));
        assert!(!info.is_dir);
        assert_eq!(info.size, 1024);
        assert_eq!(info.permissions, Some(0o644));
    }
}
