use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default)]
pub struct MemorySnapshot {
    pub mem_total: Option<u64>,
    pub mem_available: Option<u64>,
    pub swap_total: Option<u64>,
    pub swap_free: Option<u64>,
}

impl MemorySnapshot {
    pub fn mem_used(&self) -> Option<u64> {
        Some(self.mem_total?.saturating_sub(self.mem_available?))
    }

    pub fn swap_used(&self) -> Option<u64> {
        Some(self.swap_total?.saturating_sub(self.swap_free?))
    }
}

#[derive(Clone, Debug)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub uid: u32,
    pub rss_bytes: Option<u64>,
    pub cmdline: Option<String>,
}

pub struct ProcessScanEvent {
    pub scan_id: u64,
    pub memory: MemorySnapshot,
    pub processes: Vec<ProcessInfo>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminateSignal {
    Term,
    Kill,
}

#[derive(Debug, thiserror::Error)]
pub enum TerminateError {
    #[error("permission denied")]
    PermissionDenied,
    #[error("refuse to terminate self process")]
    SelfProcess,
    #[error("process not found")]
    NotFound,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("system error: {0}")]
    System(String),
}

pub async fn scan_all(
    tx: async_channel::Sender<ProcessScanEvent>,
    token: tokio_util::sync::CancellationToken,
    scan_id: u64,
) {
    let token_for_worker = token.clone();
    let analyzed = tokio::task::spawn_blocking(move || scan_all_blocking(&token_for_worker))
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("process scan worker failed: {e}");
            (MemorySnapshot::default(), Vec::new())
        });

    if token.is_cancelled() {
        return;
    }

    let (memory, processes) = analyzed;
    let event = ProcessScanEvent {
        scan_id,
        memory,
        processes,
    };
    let _ = tx.send(event).await;
}

pub fn read_memory_snapshot() -> MemorySnapshot {
    read_meminfo().unwrap_or_default()
}

fn scan_all_blocking(
    token: &tokio_util::sync::CancellationToken,
) -> (MemorySnapshot, Vec<ProcessInfo>) {
    let memory = read_meminfo().unwrap_or_default();
    let processes = scan_processes(token);
    (memory, processes)
}

pub fn current_uid() -> u32 {
    // 安全边界：结束进程仅允许同 UID；这里取当前有效 UID。
    #[cfg(unix)]
    unsafe {
        libc::geteuid()
    }
    #[cfg(not(unix))]
    {
        0
    }
}

pub fn self_pid() -> u32 {
    std::process::id()
}

pub fn can_terminate(current_uid: u32, self_pid: u32, info: &ProcessInfo) -> bool {
    info.pid != self_pid && info.uid == current_uid
}

pub fn terminate_process(
    pid: u32,
    signal: TerminateSignal,
    current_uid: u32,
    self_pid: u32,
) -> Result<(), TerminateError> {
    if pid == self_pid {
        return Err(TerminateError::SelfProcess);
    }

    let owner_uid = read_process_uid(pid)?;
    if owner_uid != current_uid {
        return Err(TerminateError::PermissionDenied);
    }

    let sig = match signal {
        TerminateSignal::Term => libc::SIGTERM,
        TerminateSignal::Kill => libc::SIGKILL,
    };

    #[cfg(unix)]
    unsafe {
        if libc::kill(pid as i32, sig) == 0 {
            return Ok(());
        }
    }
    #[cfg(not(unix))]
    {
        let _ = sig;
        return Err(TerminateError::System("unsupported platform".into()));
    }

    let err = std::io::Error::last_os_error();
    match err.raw_os_error() {
        Some(code) if code == libc::EPERM => Err(TerminateError::PermissionDenied),
        Some(code) if code == libc::ESRCH => Err(TerminateError::NotFound),
        _ => Err(TerminateError::System(err.to_string())),
    }
}

fn read_meminfo() -> Option<MemorySnapshot> {
    let raw = fs::read_to_string("/proc/meminfo").ok()?;
    Some(parse_meminfo(&raw))
}

fn parse_meminfo(contents: &str) -> MemorySnapshot {
    let mut snapshot = MemorySnapshot::default();
    for line in contents.lines() {
        let mut parts = line.split_whitespace();
        let Some(key) = parts.next() else {
            continue;
        };
        let key = key.trim_end_matches(':');
        let Some(value_str) = parts.next() else {
            continue;
        };
        let Ok(value) = value_str.parse::<u64>() else {
            continue;
        };
        let unit = parts.next().unwrap_or("");
        let bytes = match unit {
            "kB" => value.saturating_mul(1024),
            _ => value,
        };

        match key {
            "MemTotal" => snapshot.mem_total = Some(bytes),
            "MemAvailable" => snapshot.mem_available = Some(bytes),
            "SwapTotal" => snapshot.swap_total = Some(bytes),
            "SwapFree" => snapshot.swap_free = Some(bytes),
            _ => {}
        }
    }
    snapshot
}

fn scan_processes(token: &tokio_util::sync::CancellationToken) -> Vec<ProcessInfo> {
    let mut processes = Vec::new();
    let Ok(entries) = fs::read_dir("/proc") else {
        return Vec::new();
    };

    for entry in entries.filter_map(Result::ok) {
        if token.is_cancelled() {
            return Vec::new();
        }

        let file_name = entry.file_name();
        let Some(pid_str) = file_name.to_str() else {
            continue;
        };
        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };

        let status_path = proc_status_path(pid);
        let Ok(status_raw) = fs::read_to_string(&status_path) else {
            continue;
        };
        let Some(status) = parse_status(&status_raw) else {
            continue;
        };

        let cmdline = read_cmdline(pid).ok().and_then(normalize_cmdline);

        processes.push(ProcessInfo {
            pid,
            name: status.name,
            uid: status.uid,
            rss_bytes: status.rss_bytes,
            cmdline,
        });
    }

    processes.sort_by(|a, b| match (a.rss_bytes, b.rss_bytes) {
        (Some(a_s), Some(b_s)) => b_s.cmp(&a_s),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.pid.cmp(&b.pid),
    });

    processes
}

#[derive(Debug)]
struct ProcessStatus {
    name: String,
    uid: u32,
    rss_bytes: Option<u64>,
}

fn parse_status(contents: &str) -> Option<ProcessStatus> {
    let mut name: Option<String> = None;
    let mut uid: Option<u32> = None;
    let mut rss_kb: Option<u64> = None;

    for line in contents.lines() {
        if let Some(rest) = line.strip_prefix("Name:") {
            name = Some(rest.trim().to_string());
            continue;
        }

        if let Some(rest) = line.strip_prefix("Uid:") {
            uid = rest
                .split_whitespace()
                .next()
                .and_then(|v| v.parse::<u32>().ok());
            continue;
        }

        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let mut parts = rest.split_whitespace();
            rss_kb = parts.next().and_then(|v| v.parse::<u64>().ok());
        }
    }

    let name = name?;
    let uid = uid?;
    let rss_bytes = rss_kb.map(|v| v.saturating_mul(1024));

    Some(ProcessStatus {
        name,
        uid,
        rss_bytes,
    })
}

fn read_cmdline(pid: u32) -> std::io::Result<Vec<u8>> {
    fs::read(proc_cmdline_path(pid))
}

fn normalize_cmdline(raw: Vec<u8>) -> Option<String> {
    if raw.is_empty() {
        return None;
    }
    let mut s = String::from_utf8_lossy(&raw).into_owned();
    s = s.replace('\0', " ");
    let trimmed = s.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn read_process_uid(pid: u32) -> Result<u32, TerminateError> {
    let status_path = proc_status_path(pid);
    let status_raw = fs::read_to_string(&status_path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => TerminateError::NotFound,
        _ => TerminateError::Io(e),
    })?;

    let status = parse_status(&status_raw).ok_or_else(|| TerminateError::System("bad status".into()))?;
    Ok(status.uid)
}

fn proc_status_path(pid: u32) -> PathBuf {
    proc_pid_path(pid).join("status")
}

fn proc_cmdline_path(pid: u32) -> PathBuf {
    proc_pid_path(pid).join("cmdline")
}

fn proc_pid_path(pid: u32) -> PathBuf {
    Path::new("/proc").join(pid.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_meminfo_extracts_fields() {
        let raw = r#"
MemTotal:       16384256 kB
MemFree:         1000000 kB
MemAvailable:    8000000 kB
SwapTotal:       2097148 kB
SwapFree:        1048574 kB
"#;
        let s = parse_meminfo(raw);
        assert_eq!(s.mem_total, Some(16_384_256u64 * 1024));
        assert_eq!(s.mem_available, Some(8_000_000u64 * 1024));
        assert_eq!(s.swap_total, Some(2_097_148u64 * 1024));
        assert_eq!(s.swap_free, Some(1_048_574u64 * 1024));
        assert_eq!(s.mem_used(), Some((16_384_256u64 - 8_000_000u64) * 1024));
        assert_eq!(s.swap_used(), Some((2_097_148u64 - 1_048_574u64) * 1024));
    }

    #[test]
    fn parse_status_extracts_name_uid_and_rss() {
        let raw = r#"
Name:   bash
Umask:  0022
State:  S (sleeping)
Uid:    1000    1000    1000    1000
VmRSS:    12345 kB
"#;
        let s = parse_status(raw).expect("status");
        assert_eq!(s.name, "bash");
        assert_eq!(s.uid, 1000);
        assert_eq!(s.rss_bytes, Some(12_345u64 * 1024));
    }

    #[test]
    fn can_terminate_requires_same_uid_and_not_self() {
        let info = ProcessInfo {
            pid: 123,
            name: "x".into(),
            uid: 1000,
            rss_bytes: None,
            cmdline: None,
        };
        assert!(!can_terminate(1000, 123, &info));
        assert!(!can_terminate(1001, 999, &info));
        assert!(can_terminate(1000, 999, &info));
    }

    #[test]
    fn normalize_cmdline_splits_nul() {
        let raw = b"/usr/bin/python3\0-m\0http.server\0".to_vec();
        let s = normalize_cmdline(raw).expect("cmdline");
        assert_eq!(s, "/usr/bin/python3 -m http.server");
    }
}
