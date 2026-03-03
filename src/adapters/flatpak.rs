use std::future::Future;
use std::time::Instant;

use crate::adapters::util::*;
use crate::adapters::PackageAdapter;
use crate::models::{make_canonical_id, AdapterResult, Package};
use crate::subprocess::run_command;

pub struct FlatpakAdapter;

impl PackageAdapter for FlatpakAdapter {
    fn name(&self) -> &str {
        "flatpak"
    }

    fn is_available(&self) -> bool {
        command_exists("flatpak")
    }

    fn list_packages(&self) -> impl Future<Output = AdapterResult<Package>> + Send {
        async move {
            let started = Instant::now();
            let output = match run_command(
                "flatpak",
                &["list", "--columns=application,name,version,size"],
                10,
            )
            .await
            {
                Ok(o) => o,
                Err(e) => return empty_result(started, format!("flatpak: {e}")),
            };

            let mut items = Vec::new();
            let warnings = Vec::new();

            for line in output.stdout.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let mut parts = line.splitn(4, '\t');
                let (Some(app), Some(name), Some(ver), size_text) =
                    (parts.next(), parts.next(), parts.next(), parts.next())
                else {
                    continue;
                };

                let app = app.trim();
                if app.is_empty() {
                    continue;
                }

                let display = if name.trim().is_empty() {
                    app
                } else {
                    name.trim()
                };
                let size = size_text.and_then(parse_human_size_to_bytes);

                items.push(Package {
                    canonical_id: make_canonical_id("flatpak", app),
                    name: display.to_string(),
                    version: ver.trim().to_string(),
                    source: "flatpak".to_string(),
                    install_method: "flatpak".to_string(),
                    install_path: Some(format!("/var/lib/flatpak/app/{}", app)),
                    uninstall_command: Some(format!("flatpak uninstall {}", app)),
                    size,
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
