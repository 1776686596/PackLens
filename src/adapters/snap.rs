use std::future::Future;
use std::time::Instant;

use crate::adapters::util::*;
use crate::adapters::PackageAdapter;
use crate::models::{make_canonical_id, AdapterResult, Package};
use crate::subprocess::run_command;

pub struct SnapAdapter;

impl PackageAdapter for SnapAdapter {
    fn name(&self) -> &str {
        "snap"
    }

    fn is_available(&self) -> bool {
        command_exists("snap")
    }

    fn list_packages(&self) -> impl Future<Output = AdapterResult<Package>> + Send {
        async move {
            let started = Instant::now();
            let output = match run_command("snap", &["list"], 10).await {
                Ok(o) => o,
                Err(e) => return empty_result(started, format!("snap: {e}")),
            };

            let mut items = Vec::new();
            let warnings = Vec::new();

            for (i, line) in output.stdout.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if i == 0 && trimmed.starts_with("Name") {
                    continue;
                }

                let cols: Vec<&str> = trimmed.split_whitespace().collect();
                if cols.len() < 2 {
                    continue;
                }

                items.push(Package {
                    canonical_id: make_canonical_id("snap", cols[0]),
                    name: cols[0].to_string(),
                    version: cols[1].to_string(),
                    source: "snap".to_string(),
                    install_method: "snap".to_string(),
                    install_path: Some(format!("/snap/{}", cols[0])),
                    uninstall_command: Some(format!("sudo snap remove {}", cols[0])),
                    size: None,
                    description: String::new(),
                    icon_name: None,
                    desktop_file: None,
                });
            }

            AdapterResult {
                items,
                warnings,
                duration_ms: elapsed_ms(&started),
                timestamp: now_timestamp(),
            }
        }
    }
}
