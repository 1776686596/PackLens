use std::future::Future;

use crate::adapters::util::{command_exists, first_non_empty_line, resolve_path};
use crate::adapters::EnvironmentAdapter;
use crate::models::{detect_install_method, GlobalPackageInfo, RuntimeInfo, VersionManagerInfo};
use crate::subprocess::run_command;

pub struct JavaEnvAdapter;

impl EnvironmentAdapter for JavaEnvAdapter {
    fn name(&self) -> &str {
        "java"
    }

    fn detect_runtimes(&self) -> impl Future<Output = Vec<RuntimeInfo>> + Send {
        async move {
            let mut runtimes = Vec::new();
            if command_exists("java") {
                if let Some(rt) = detect_java_like("java", "java").await {
                    runtimes.push(rt);
                }
            }
            if command_exists("javac") {
                if let Some(rt) = detect_java_like("javac", "javac").await {
                    runtimes.push(rt);
                }
            }
            runtimes
        }
    }

    fn detect_version_managers(&self) -> impl Future<Output = Vec<VersionManagerInfo>> + Send {
        async move { Vec::new() }
    }

    fn list_global_packages(&self) -> impl Future<Output = Vec<GlobalPackageInfo>> + Send {
        async move { Vec::new() }
    }
}

async fn detect_java_like(binary: &str, language: &str) -> Option<RuntimeInfo> {
    let output = match run_command(binary, &["--version"], 5).await {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!("{binary} --version: {e}");
            return None;
        }
    };
    let line = first_non_empty_line(&output.stdout, &output.stderr);
    let version = parse_java_version(line)?;
    let path = resolve_path(binary).await.unwrap_or_default();
    Some(RuntimeInfo {
        language: language.into(),
        version,
        install_method: detect_install_method(&path).into(),
        path,
    })
}

fn parse_java_version(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    let v = if parts[1].eq_ignore_ascii_case("version") {
        parts.get(2)?
    } else {
        &parts[1]
    };
    let version = v.trim_matches('"').to_string();
    if version.is_empty() {
        None
    } else {
        Some(version)
    }
}
