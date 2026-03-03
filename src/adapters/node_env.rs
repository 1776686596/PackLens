use std::collections::HashMap;
use std::future::Future;
use std::path::Path;

use serde::Deserialize;

use crate::adapters::util::{command_exists, first_non_empty_line, resolve_path};
use crate::adapters::EnvironmentAdapter;
use crate::models::{
    detect_install_method, GlobalPackageInfo, ManagedVersion, RuntimeInfo, VersionManagerInfo,
};
use crate::subprocess::run_command;

#[derive(Deserialize)]
struct NpmList {
    dependencies: Option<HashMap<String, NpmDep>>,
}

#[derive(Deserialize)]
struct NpmDep {
    version: Option<String>,
}

pub struct NodeEnvAdapter;

impl EnvironmentAdapter for NodeEnvAdapter {
    fn name(&self) -> &str {
        "node"
    }

    fn detect_runtimes(&self) -> impl Future<Output = Vec<RuntimeInfo>> + Send {
        async move {
            if !command_exists("node") {
                return Vec::new();
            }
            let output = match run_command("node", &["--version"], 5).await {
                Ok(o) => o,
                Err(e) => {
                    tracing::warn!("node --version: {e}");
                    return Vec::new();
                }
            };
            let line = first_non_empty_line(&output.stdout, &output.stderr);
            let version = line.trim_start_matches('v').to_string();
            if version.is_empty() {
                return Vec::new();
            }
            let path = resolve_path("node").await.unwrap_or_default();
            vec![RuntimeInfo {
                language: "node".into(),
                version,
                install_method: detect_install_method(&path).into(),
                path,
            }]
        }
    }

    fn detect_version_managers(&self) -> impl Future<Output = Vec<VersionManagerInfo>> + Send {
        async move {
            let home = match std::env::var("HOME") {
                Ok(h) => h,
                Err(_) => return Vec::new(),
            };
            let nvm_script = format!("{home}/.nvm/nvm.sh");
            if !Path::new(&nvm_script).exists() {
                return Vec::new();
            }

            let output = match run_command(
                "bash",
                &["-c", "source \"$HOME/.nvm/nvm.sh\" && nvm list"],
                5,
            )
            .await
            {
                Ok(o) => o,
                Err(e) => {
                    tracing::warn!("nvm list: {e}");
                    return Vec::new();
                }
            };

            vec![VersionManagerInfo {
                name: "nvm".into(),
                managed_versions: parse_nvm_versions(&output.stdout),
                path: nvm_script,
            }]
        }
    }

    fn list_global_packages(&self) -> impl Future<Output = Vec<GlobalPackageInfo>> + Send {
        async move {
            if !command_exists("npm") {
                return Vec::new();
            }
            let output = match run_command("npm", &["list", "-g", "--json", "--depth=0"], 5).await {
                Ok(o) => o,
                Err(e) => {
                    tracing::warn!("npm list -g: {e}");
                    return Vec::new();
                }
            };
            let parsed: NpmList = match serde_json::from_str(&output.stdout) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("npm json parse: {e}");
                    return Vec::new();
                }
            };
            let mut pkgs: Vec<GlobalPackageInfo> = parsed
                .dependencies
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(name, dep)| {
                    dep.version.map(|v| GlobalPackageInfo {
                        manager: "npm".into(),
                        name,
                        version: v,
                    })
                })
                .collect();
            pkgs.sort_by(|a, b| a.name.cmp(&b.name));
            pkgs
        }
    }
}

fn parse_nvm_versions(raw: &str) -> Vec<ManagedVersion> {
    let mut versions = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.contains('v') {
            continue;
        }

        let active = trimmed.starts_with("->");
        let candidate = trimmed.trim_start_matches("->").trim();
        let token = candidate.split_whitespace().next().unwrap_or("");
        if !token.starts_with('v') {
            continue;
        }
        if !token.as_bytes().get(1).is_some_and(|b| b.is_ascii_digit()) {
            continue;
        }

        let version = token.to_string();
        if !versions
            .iter()
            .any(|v: &ManagedVersion| v.version == version)
        {
            versions.push(ManagedVersion { version, active });
        }
    }
    versions
}
