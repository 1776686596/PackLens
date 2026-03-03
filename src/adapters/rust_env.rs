use std::future::Future;

use crate::adapters::util::{command_exists, first_non_empty_line, resolve_path};
use crate::adapters::EnvironmentAdapter;
use crate::models::{
    detect_install_method, GlobalPackageInfo, ManagedVersion, RuntimeInfo, VersionManagerInfo,
};
use crate::subprocess::run_command;

pub struct RustEnvAdapter;

impl EnvironmentAdapter for RustEnvAdapter {
    fn name(&self) -> &str {
        "rust"
    }

    fn detect_runtimes(&self) -> impl Future<Output = Vec<RuntimeInfo>> + Send {
        async move {
            if !command_exists("rustc") {
                return Vec::new();
            }
            let output = match run_command("rustc", &["--version"], 5).await {
                Ok(o) => o,
                Err(e) => {
                    tracing::warn!("rustc --version: {e}");
                    return Vec::new();
                }
            };
            let line = first_non_empty_line(&output.stdout, &output.stderr);
            let version = match line.strip_prefix("rustc ") {
                Some(rest) => rest.split_whitespace().next().unwrap_or("").to_string(),
                None => {
                    tracing::warn!("unexpected rustc version: {line}");
                    return Vec::new();
                }
            };
            if version.is_empty() {
                return Vec::new();
            }
            let path = resolve_path("rustc").await.unwrap_or_default();
            vec![RuntimeInfo {
                language: "rust".into(),
                version,
                install_method: detect_install_method(&path).into(),
                path,
            }]
        }
    }

    fn detect_version_managers(&self) -> impl Future<Output = Vec<VersionManagerInfo>> + Send {
        async move {
            if !command_exists("rustup") {
                return Vec::new();
            }
            let output = match run_command("rustup", &["show"], 5).await {
                Ok(o) => o,
                Err(e) => {
                    tracing::warn!("rustup show: {e}");
                    return Vec::new();
                }
            };
            vec![VersionManagerInfo {
                name: "rustup".into(),
                managed_versions: parse_rustup_toolchains(&output.stdout),
                path: resolve_path("rustup").await.unwrap_or_default(),
            }]
        }
    }

    fn list_global_packages(&self) -> impl Future<Output = Vec<GlobalPackageInfo>> + Send {
        async move {
            if !command_exists("cargo") {
                return Vec::new();
            }
            let output = match run_command("cargo", &["install", "--list"], 5).await {
                Ok(o) => o,
                Err(e) => {
                    tracing::warn!("cargo install --list: {e}");
                    return Vec::new();
                }
            };
            output
                .stdout
                .lines()
                .filter_map(|line| {
                    if line.starts_with(' ') || line.starts_with('\t') || line.trim().is_empty() {
                        return None;
                    }
                    let header = line.trim_end().trim_end_matches(':');
                    let mut parts = header.split_whitespace();
                    let name = parts.next()?;
                    let ver = parts.next()?.trim_start_matches('v');
                    if ver.is_empty() {
                        return None;
                    }
                    Some(GlobalPackageInfo {
                        manager: "cargo".into(),
                        name: name.into(),
                        version: ver.into(),
                    })
                })
                .collect()
        }
    }
}

fn parse_rustup_toolchains(raw: &str) -> Vec<ManagedVersion> {
    let mut versions = Vec::new();
    let mut in_toolchains = false;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("installed toolchains")
            || trimmed.starts_with("Installed Toolchains")
        {
            in_toolchains = true;
            continue;
        }
        if trimmed.starts_with("active toolchain") || trimmed.starts_with("Active Toolchain") {
            break;
        }
        if !in_toolchains || trimmed.is_empty() || trimmed.starts_with("---") {
            continue;
        }

        let active = trimmed.contains("(default)");
        let version = trimmed
            .replace("(default)", "")
            .replace("(override)", "")
            .trim()
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();

        if !version.is_empty() && version.contains('-') {
            versions.push(ManagedVersion { version, active });
        }
    }
    versions
}
