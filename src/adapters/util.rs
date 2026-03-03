use crate::models::AdapterResult;
use crate::subprocess::run_command;
use std::fs;
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub fn command_exists(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn now_timestamp() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

pub fn elapsed_ms(started: &Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

pub fn empty_result<T>(started: Instant, warning: String) -> AdapterResult<T> {
    AdapterResult {
        items: Vec::new(),
        warnings: vec![warning],
        duration_ms: elapsed_ms(&started),
        timestamp: now_timestamp(),
    }
}

pub async fn resolve_path(binary: &str) -> Option<String> {
    run_command("which", &[binary], 5).await.ok().and_then(|o| {
        let p = o.stdout.trim().to_string();
        if p.is_empty() {
            None
        } else {
            Some(p)
        }
    })
}

pub fn first_non_empty_line<'a>(stdout: &'a str, stderr: &'a str) -> &'a str {
    stdout
        .lines()
        .chain(stderr.lines())
        .find(|l| !l.trim().is_empty())
        .map_or("", str::trim)
}

pub fn parse_human_size_to_bytes(input: &str) -> Option<u64> {
    let normalized = input
        .trim()
        .replace('\u{a0}', " ")
        .replace("bytes", "B")
        .replace("byte", "B");
    if normalized.is_empty() || normalized == "-" || normalized == "?" {
        return None;
    }

    let compact: String = normalized
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect();
    if compact.is_empty() {
        return None;
    }

    let number_end = compact
        .find(|ch: char| !(ch.is_ascii_digit() || ch == '.' || ch == ','))
        .unwrap_or(compact.len());
    let number = compact[..number_end].replace(',', "");
    if number.is_empty() {
        return None;
    }

    let value = number.parse::<f64>().ok()?;
    let unit = compact[number_end..].to_ascii_lowercase();
    let multiplier = match unit.as_str() {
        "" | "b" => 1.0,
        "k" | "kb" | "kib" => 1024.0,
        "m" | "mb" | "mib" => 1024.0 * 1024.0,
        "g" | "gb" | "gib" => 1024.0 * 1024.0 * 1024.0,
        "t" | "tb" | "tib" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };

    Some((value * multiplier) as u64)
}

pub fn file_size_if_regular(path: &str) -> Option<u64> {
    let metadata = fs::metadata(path).ok()?;
    if metadata.is_file() {
        Some(metadata.len())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_human_size_plain_bytes() {
        assert_eq!(parse_human_size_to_bytes("1024"), Some(1024));
        assert_eq!(parse_human_size_to_bytes("2048B"), Some(2048));
    }

    #[test]
    fn parse_human_size_with_units() {
        assert_eq!(parse_human_size_to_bytes("1.5 MB"), Some(1_572_864));
        assert_eq!(parse_human_size_to_bytes("2 GiB"), Some(2_147_483_648));
        assert_eq!(parse_human_size_to_bytes("1,024 kB"), Some(1_048_576));
    }

    #[test]
    fn parse_human_size_invalid() {
        assert_eq!(parse_human_size_to_bytes("-"), None);
        assert_eq!(parse_human_size_to_bytes("unknown"), None);
    }
}
