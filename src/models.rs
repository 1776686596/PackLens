pub struct Package {
    pub canonical_id: String,
    pub name: String,
    pub version: String,
    pub source: String,
    pub install_method: String,
    pub install_path: Option<String>,
    pub uninstall_command: Option<String>,
    pub size: Option<u64>,
    pub description: String,
    pub icon_name: Option<String>,
    pub desktop_file: Option<String>,
}

pub struct AdapterResult<T> {
    pub items: Vec<T>,
    pub warnings: Vec<String>,
    pub duration_ms: u64,
    pub timestamp: f64,
}

pub struct RuntimeInfo {
    pub language: String,
    pub version: String,
    pub path: String,
    pub install_method: String,
}

pub struct VersionManagerInfo {
    pub name: String,
    pub managed_versions: Vec<ManagedVersion>,
    pub path: String,
}

pub struct ManagedVersion {
    pub version: String,
    pub active: bool,
}

pub struct GlobalPackageInfo {
    pub manager: String,
    pub name: String,
    pub version: String,
}

#[derive(Clone)]
pub struct CacheInfo {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub requires_sudo: bool,
}

#[derive(Clone)]
pub struct CleanupSuggestion {
    pub description: String,
    pub estimated_bytes: u64,
    pub command: String,
    pub requires_sudo: bool,
    pub risk_level: RiskLevel,
}

#[derive(Clone, Copy)]
pub enum RiskLevel {
    Safe,
    Moderate,
}

const CLEANUP_WHITELIST: &[&str] = &[
    "apt clean",
    "pip3 cache purge",
    "npm cache clean --force",
    "conda clean --all -y",
    "cargo cache --autoclean",
    "docker system prune -f",
];

impl CleanupSuggestion {
    pub fn new(
        description: String,
        estimated_bytes: u64,
        command: String,
        requires_sudo: bool,
        risk_level: RiskLevel,
    ) -> Option<Self> {
        if !CLEANUP_WHITELIST.contains(&command.as_str()) {
            tracing::warn!("cleanup command not in whitelist: {command}");
            return None;
        }
        Some(Self {
            description,
            estimated_bytes,
            command,
            requires_sudo,
            risk_level,
        })
    }
}

pub fn make_canonical_id(source: &str, name: &str) -> String {
    format!("{source}:{name}")
}

pub fn parse_canonical_id(id: &str) -> (&str, &str) {
    let mut parts = id.splitn(2, ':');
    let source = parts.next().expect("canonical_id missing source");
    let name = parts.next().expect("canonical_id missing name");
    (source, name)
}

pub fn detect_install_method(path: &str) -> &'static str {
    if path.contains("/.nvm/") {
        "nvm"
    } else if path.contains("/.rustup/") {
        "rustup"
    } else if path.contains("/anaconda3/") || path.contains("/miniconda3/") {
        "conda"
    } else if path.contains("/.cargo/bin/") {
        "cargo"
    } else if path.starts_with("/usr/local/bin/") {
        "manual"
    } else if path.starts_with("/usr/bin/") || path.starts_with("/bin/") {
        "apt"
    } else if path.contains("/.local/bin/") {
        "pipx"
    } else {
        "unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_id_roundtrip() {
        let id = make_canonical_id("apt", "vim");
        assert_eq!(id, "apt:vim");
        let (source, name) = parse_canonical_id(&id);
        assert_eq!(source, "apt");
        assert_eq!(name, "vim");
    }

    #[test]
    fn canonical_id_with_colon_in_name() {
        let id = make_canonical_id("apt", "libc6:amd64");
        assert_eq!(id, "apt:libc6:amd64");
        let (source, name) = parse_canonical_id(&id);
        assert_eq!(source, "apt");
        assert_eq!(name, "libc6:amd64");
    }

    #[test]
    fn install_method_detection() {
        assert_eq!(
            detect_install_method("/home/u/.nvm/versions/node/v20/bin/node"),
            "nvm"
        );
        assert_eq!(
            detect_install_method("/home/u/.rustup/toolchains/stable/bin/rustc"),
            "rustup"
        );
        assert_eq!(
            detect_install_method("/home/u/anaconda3/bin/python"),
            "conda"
        );
        assert_eq!(detect_install_method("/home/u/.cargo/bin/cargo"), "cargo");
        assert_eq!(detect_install_method("/usr/local/bin/myapp"), "manual");
        assert_eq!(detect_install_method("/usr/bin/python3"), "apt");
        assert_eq!(detect_install_method("/home/u/.local/bin/pipx"), "pipx");
        assert_eq!(detect_install_method("/opt/custom/bin/tool"), "unknown");
    }

    #[test]
    fn cleanup_whitelist_accepts_valid() {
        assert!(
            CleanupSuggestion::new("t".into(), 1, "apt clean".into(), true, RiskLevel::Safe)
                .is_some()
        );
        assert!(CleanupSuggestion::new(
            "t".into(),
            1,
            "pip3 cache purge".into(),
            false,
            RiskLevel::Safe
        )
        .is_some());
        assert!(CleanupSuggestion::new(
            "t".into(),
            1,
            "docker system prune -f".into(),
            false,
            RiskLevel::Moderate
        )
        .is_some());
    }

    #[test]
    fn cleanup_whitelist_rejects_invalid() {
        assert!(
            CleanupSuggestion::new("t".into(), 1, "rm -rf /".into(), false, RiskLevel::Safe)
                .is_none()
        );
    }

    #[test]
    fn sort_packages_by_size() {
        let pkgs = vec![
            Package {
                canonical_id: "a:1".into(),
                name: "a".into(),
                version: String::new(),
                source: "a".into(),
                install_method: "a".into(),
                install_path: None,
                uninstall_command: None,
                size: Some(100),
                description: String::new(),
                icon_name: None,
                desktop_file: None,
            },
            Package {
                canonical_id: "a:2".into(),
                name: "b".into(),
                version: String::new(),
                source: "a".into(),
                install_method: "a".into(),
                install_path: None,
                uninstall_command: None,
                size: Some(500),
                description: String::new(),
                icon_name: None,
                desktop_file: None,
            },
            Package {
                canonical_id: "a:3".into(),
                name: "c".into(),
                version: String::new(),
                source: "a".into(),
                install_method: "a".into(),
                install_path: None,
                uninstall_command: None,
                size: None,
                description: String::new(),
                icon_name: None,
                desktop_file: None,
            },
        ];
        let mut sorted: Vec<&Package> = pkgs.iter().collect();
        sorted.sort_by(|a, b| match (a.size, b.size) {
            (Some(a_s), Some(b_s)) => b_s.cmp(&a_s),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });
        assert_eq!(sorted[0].name, "b");
        assert_eq!(sorted[1].name, "a");
        assert_eq!(sorted[2].name, "c");
    }
}
