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
struct CondaEnvList {
    envs: Vec<String>,
}

#[derive(Deserialize)]
struct PipPkg {
    name: String,
    version: String,
}

pub struct PythonEnvAdapter;

impl EnvironmentAdapter for PythonEnvAdapter {
    fn name(&self) -> &str {
        "python"
    }

    fn detect_runtimes(&self) -> impl Future<Output = Vec<RuntimeInfo>> + Send {
        async move {
            if !command_exists("python3") {
                return Vec::new();
            }
            let output = match run_command("python3", &["--version"], 5).await {
                Ok(o) => o,
                Err(e) => {
                    tracing::warn!("python3 --version: {e}");
                    return Vec::new();
                }
            };
            let line = first_non_empty_line(&output.stdout, &output.stderr);
            let version = match line.strip_prefix("Python ") {
                Some(v) => v.to_string(),
                None => {
                    tracing::warn!("unexpected python version: {line}");
                    return Vec::new();
                }
            };
            let path = resolve_path("python3").await.unwrap_or_default();
            vec![RuntimeInfo {
                language: "python".into(),
                version,
                install_method: detect_install_method(&path).into(),
                path,
            }]
        }
    }

    fn detect_version_managers(&self) -> impl Future<Output = Vec<VersionManagerInfo>> + Send {
        async move {
            let mut managers = Vec::new();

            if command_exists("conda") {
                if let Ok(output) = run_command("conda", &["env", "list", "--json"], 5).await {
                    let versions = serde_json::from_str::<CondaEnvList>(&output.stdout)
                        .map(|e| {
                            e.envs
                                .into_iter()
                                .map(|p| ManagedVersion {
                                    version: conda_env_name(&p),
                                    active: false,
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    managers.push(VersionManagerInfo {
                        name: "conda".into(),
                        managed_versions: versions,
                        path: resolve_path("conda").await.unwrap_or_default(),
                    });
                }
            }

            if command_exists("uv") {
                if run_command("uv", &["--version"], 5).await.is_ok() {
                    managers.push(VersionManagerInfo {
                        name: "uv".into(),
                        managed_versions: Vec::new(),
                        path: resolve_path("uv").await.unwrap_or_default(),
                    });
                }
            }

            managers
        }
    }

    fn list_global_packages(&self) -> impl Future<Output = Vec<GlobalPackageInfo>> + Send {
        async move {
            let mut packages = Vec::<GlobalPackageInfo>::new();

            if command_exists("pip3") {
                match run_command("pip3", &["list", "--format=json"], 5).await {
                    Ok(output) => {
                        let parsed = serde_json::from_str::<Vec<PipPkg>>(&output.stdout)
                            .map(|pkgs| {
                                pkgs.into_iter()
                                    .map(|p| GlobalPackageInfo {
                                        manager: "pip".into(),
                                        name: p.name,
                                        version: p.version,
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_else(|e| {
                                tracing::warn!("pip json parse: {e}");
                                Vec::new()
                            });
                        packages.extend(parsed);
                    }
                    Err(e) => tracing::warn!("pip3 list: {e}"),
                }
            }

            if command_exists("uv") {
                match run_command("uv", &["tool", "list"], 10).await {
                    Ok(output) => packages.extend(parse_uv_tool_list(&output.stdout)),
                    Err(e) => tracing::warn!("uv tool list: {e}"),
                }
            }

            if command_exists("pipx") {
                match run_command("pipx", &["list", "--json"], 10).await {
                    Ok(output) => packages.extend(parse_pipx_list_json(&output.stdout)),
                    Err(e) => tracing::warn!("pipx list --json: {e}"),
                }
            }

            normalize_global_packages(packages)
        }
    }
}

fn conda_env_name(path: &str) -> String {
    if path.ends_with("/anaconda3") || path.ends_with("/miniconda3") {
        "base".into()
    } else {
        Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path)
            .to_string()
    }
}

fn parse_uv_tool_list(raw: &str) -> Vec<GlobalPackageInfo> {
    let mut items = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('-') || trimmed.starts_with('[') {
            continue;
        }

        let mut parts = trimmed.split_whitespace();
        let Some(name) = parts.next() else {
            continue;
        };
        if name.eq_ignore_ascii_case("tool") || name.eq_ignore_ascii_case("name") {
            continue;
        }

        let mut version = None::<String>;
        for token in parts {
            if let Some(v) = normalize_version_token(token) {
                version = Some(v);
                break;
            }
        }

        let Some(version) = version else {
            continue;
        };

        items.push(GlobalPackageInfo {
            manager: "uv".into(),
            name: name.to_string(),
            version,
        });
    }
    items
}

fn parse_pipx_list_json(raw: &str) -> Vec<GlobalPackageInfo> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return Vec::new();
    };

    let Some(venvs) = value.get("venvs").and_then(|v| v.as_object()) else {
        return Vec::new();
    };

    let mut items = Vec::new();
    for (name, detail) in venvs {
        let version = detail
            .get("metadata")
            .and_then(|m| m.get("main_package"))
            .and_then(|p| p.get("package_version"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        items.push(GlobalPackageInfo {
            manager: "pipx".into(),
            name: name.to_string(),
            version,
        });
    }
    items
}

fn normalize_version_token(token: &str) -> Option<String> {
    let trimmed = token
        .trim()
        .trim_end_matches(',')
        .trim_end_matches(':')
        .trim_end_matches(')');
    let normalized = trimmed.trim_start_matches('v');
    let mut chars = normalized.chars();
    let Some(first) = chars.next() else {
        return None;
    };
    if !first.is_ascii_digit() {
        return None;
    }
    Some(normalized.to_string())
}

fn normalize_global_packages(mut packages: Vec<GlobalPackageInfo>) -> Vec<GlobalPackageInfo> {
    packages.sort_by(|a, b| {
        a.manager
            .cmp(&b.manager)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.version.cmp(&b.version))
    });
    packages.dedup_by(|a, b| a.manager == b.manager && a.name == b.name);
    packages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_uv_tool_list_skips_headers_and_extracts_versions() {
        let raw = r#"
Tool Version
---- -------
ruff v0.6.0
black 24.2.0
"#;

        let pkgs = parse_uv_tool_list(raw);
        assert!(pkgs
            .iter()
            .any(|p| p.manager == "uv" && p.name == "ruff" && p.version == "0.6.0"));
        assert!(pkgs
            .iter()
            .any(|p| p.manager == "uv" && p.name == "black" && p.version == "24.2.0"));
        assert!(!pkgs.iter().any(|p| p.name.eq_ignore_ascii_case("tool")));
    }

    #[test]
    fn parse_pipx_list_json_extracts_version() {
        let raw = r#"
{
  "venvs": {
    "black": {
      "metadata": {
        "main_package": {
          "package_version": "24.2.0"
        }
      }
    }
  }
}
"#;

        let pkgs = parse_pipx_list_json(raw);
        assert!(pkgs
            .iter()
            .any(|p| p.manager == "pipx" && p.name == "black" && p.version == "24.2.0"));
    }

    #[test]
    fn normalize_global_packages_sorts_and_dedups() {
        let pkgs = vec![
            GlobalPackageInfo {
                manager: "pipx".into(),
                name: "b".into(),
                version: "1.0.0".into(),
            },
            GlobalPackageInfo {
                manager: "pip".into(),
                name: "a".into(),
                version: "2.0.0".into(),
            },
            GlobalPackageInfo {
                manager: "pip".into(),
                name: "a".into(),
                version: "3.0.0".into(),
            },
        ];

        let out = normalize_global_packages(pkgs);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].manager, "pip");
        assert_eq!(out[0].name, "a");
        assert_eq!(out[1].manager, "pipx");
        assert_eq!(out[1].name, "b");
    }
}
