use std::future::Future;
use std::time::Instant;

use crate::adapters::util::*;
use crate::adapters::PackageAdapter;
use crate::models::{make_canonical_id, AdapterResult, Package};
use crate::subprocess::run_command;

pub struct AptAdapter;

impl PackageAdapter for AptAdapter {
    fn name(&self) -> &str {
        "apt"
    }

    fn is_available(&self) -> bool {
        command_exists("dpkg-query")
    }

    fn list_packages(&self) -> impl Future<Output = AdapterResult<Package>> + Send {
        async move {
            let started = Instant::now();
            let output = match run_command(
                "dpkg-query",
                &[
                    "-W",
                    "-f=${Package}\t${Version}\t${Installed-Size}\t${Description}\n",
                ],
                10,
            )
            .await
            {
                Ok(o) => o,
                Err(e) => return empty_result(started, format!("apt: {e}")),
            };

            let mut items = Vec::new();
            let warnings = Vec::new();

            for line in output.stdout.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                let mut parts = line.splitn(4, '\t');
                let (Some(pkg), Some(ver), Some(sz), Some(desc)) =
                    (parts.next(), parts.next(), parts.next(), parts.next())
                else {
                    continue;
                };

                let pkg = pkg.trim();
                if pkg.is_empty() {
                    continue;
                }

                let size = sz
                    .trim()
                    .parse::<u64>()
                    .ok()
                    .map(|kb| kb.saturating_mul(1024));

                items.push(Package {
                    canonical_id: make_canonical_id("apt", pkg),
                    name: pkg.to_string(),
                    version: ver.trim().to_string(),
                    source: "apt".to_string(),
                    install_method: "apt".to_string(),
                    install_path: Some("/usr".to_string()),
                    uninstall_command: Some(format!("sudo apt remove {}", pkg)),
                    size,
                    description: desc.trim().to_string(),
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
