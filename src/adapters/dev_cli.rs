use std::collections::HashMap;
use std::future::Future;
use std::time::Instant;

use serde::Deserialize;

use crate::adapters::util::*;
use crate::adapters::PackageAdapter;
use crate::models::{make_canonical_id, AdapterResult, Package};
use crate::subprocess::run_command;

#[derive(Deserialize)]
struct NpmList {
    dependencies: Option<HashMap<String, NpmDep>>,
}

#[derive(Deserialize)]
struct NpmDep {
    version: Option<String>,
}

pub struct DevCliAdapter;

impl PackageAdapter for DevCliAdapter {
    fn name(&self) -> &str {
        "dev-cli"
    }

    fn is_available(&self) -> bool {
        command_exists("npm")
            || command_exists("cargo")
            || command_exists("uv")
            || command_exists("pipx")
    }

    fn list_packages(&self) -> impl Future<Output = AdapterResult<Package>> + Send {
        async move {
            let started = Instant::now();
            let mut items = Vec::new();
            let mut warnings = Vec::new();

            if command_exists("npm") {
                match run_command("npm", &["list", "-g", "--json", "--depth=0"], 10).await {
                    Ok(output) => match serde_json::from_str::<NpmList>(&output.stdout) {
                        Ok(parsed) => {
                            let mut npm_items: Vec<Package> = parsed
                                .dependencies
                                .unwrap_or_default()
                                .into_iter()
                                .filter_map(|(name, dep)| {
                                    dep.version.map(|version| Package {
                                        canonical_id: make_canonical_id("npm", &name),
                                        name: name.clone(),
                                        version,
                                        source: "npm".into(),
                                        install_method: "npm".into(),
                                        install_path: None,
                                        uninstall_command: Some(format!(
                                            "npm uninstall -g {}",
                                            name
                                        )),
                                        size: None,
                                        description: String::new(),
                                        icon_name: None,
                                        desktop_file: None,
                                    })
                                })
                                .collect();
                            npm_items.sort_by(|a, b| a.name.cmp(&b.name));
                            items.extend(npm_items);
                        }
                        Err(e) => warnings.push(format!("npm: json parse failed: {e}")),
                    },
                    Err(e) => warnings.push(format!("npm: {e}")),
                }
            }

            if command_exists("cargo") {
                match run_command("cargo", &["install", "--list"], 10).await {
                    Ok(output) => {
                        for line in output.stdout.lines() {
                            if line.starts_with(' ')
                                || line.starts_with('\t')
                                || line.trim().is_empty()
                            {
                                continue;
                            }
                            let header = line.trim_end().trim_end_matches(':');
                            let mut parts = header.split_whitespace();
                            let Some(name) = parts.next() else {
                                continue;
                            };
                            let Some(ver) = parts.next() else {
                                continue;
                            };
                            let version = ver.trim_start_matches('v');
                            if version.is_empty() {
                                continue;
                            }
                            items.push(Package {
                                canonical_id: make_canonical_id("cargo", name),
                                name: name.to_string(),
                                version: version.to_string(),
                                source: "cargo".into(),
                                install_method: "cargo".into(),
                                install_path: None,
                                uninstall_command: Some(format!("cargo uninstall {name}")),
                                size: None,
                                description: String::new(),
                                icon_name: None,
                                desktop_file: None,
                            });
                        }
                    }
                    Err(e) => warnings.push(format!("cargo: {e}")),
                }
            }

            if command_exists("uv") {
                match run_command("uv", &["tool", "list"], 10).await {
                    Ok(output) => {
                        for line in output.stdout.lines() {
                            let trimmed = line.trim();
                            if trimmed.is_empty()
                                || trimmed.starts_with('-')
                                || trimmed.starts_with('[')
                            {
                                continue;
                            }
                            let mut parts = trimmed.split_whitespace();
                            let Some(name) = parts.next() else {
                                continue;
                            };
                            let version = parts
                                .find(|p| {
                                    p.starts_with('v')
                                        || p.chars().next().is_some_and(|c| c.is_ascii_digit())
                                })
                                .unwrap_or("")
                                .trim_start_matches('v')
                                .to_string();
                            items.push(Package {
                                canonical_id: make_canonical_id("uv", name),
                                name: name.to_string(),
                                version,
                                source: "uv".into(),
                                install_method: "uv".into(),
                                install_path: None,
                                uninstall_command: Some(format!("uv tool uninstall {name}")),
                                size: None,
                                description: String::new(),
                                icon_name: None,
                                desktop_file: None,
                            });
                        }
                    }
                    Err(e) => warnings.push(format!("uv: {e}")),
                }
            }

            if command_exists("pipx") {
                match run_command("pipx", &["list", "--json"], 10).await {
                    Ok(output) => match serde_json::from_str::<serde_json::Value>(&output.stdout) {
                        Ok(value) => {
                            let apps = value
                                .get("venvs")
                                .and_then(|v| v.as_object())
                                .cloned()
                                .unwrap_or_default();
                            for (name, detail) in apps {
                                let version = detail
                                    .get("metadata")
                                    .and_then(|m| m.get("main_package"))
                                    .and_then(|p| p.get("package_version"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                items.push(Package {
                                    canonical_id: make_canonical_id("pipx", &name),
                                    name: name.clone(),
                                    version,
                                    source: "pipx".into(),
                                    install_method: "pipx".into(),
                                    install_path: None,
                                    uninstall_command: Some(format!("pipx uninstall {name}")),
                                    size: None,
                                    description: String::new(),
                                    icon_name: None,
                                    desktop_file: None,
                                });
                            }
                        }
                        Err(e) => warnings.push(format!("pipx: json parse failed: {e}")),
                    },
                    Err(e) => warnings.push(format!("pipx: {e}")),
                }
            }

            items.sort_by(|a, b| a.canonical_id.cmp(&b.canonical_id));
            items.dedup_by(|a, b| a.canonical_id == b.canonical_id);

            AdapterResult {
                items,
                warnings,
                duration_ms: elapsed_ms(&started),
                timestamp: now_timestamp(),
            }
        }
    }
}
