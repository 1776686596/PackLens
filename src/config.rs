use serde::Deserialize;
use tracing::{debug, warn};

#[derive(Debug, Deserialize)]
struct RawConfig {
    show_all_packages: Option<bool>,
    top_n: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub show_all_packages: bool,
    pub top_n: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            show_all_packages: false,
            top_n: 50,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let path = dirs_path();
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("config file not found, using defaults");
                return Self::default();
            }
            Err(e) => {
                warn!("failed to read config file: {e}");
                return Self::default();
            }
        };

        let raw: RawConfig = match toml::from_str(&content) {
            Ok(r) => r,
            Err(e) => {
                warn!("failed to parse config: {e}");
                return Self::default();
            }
        };

        let defaults = Self::default();
        Self {
            show_all_packages: raw.show_all_packages.unwrap_or(defaults.show_all_packages),
            top_n: raw.top_n.map_or(defaults.top_n, |n| n.clamp(10, 200)),
        }
    }
}

fn dirs_path() -> std::path::PathBuf {
    let mut path = dirs_config_dir();
    path.push("config.toml");
    path
}

fn dirs_config_dir() -> std::path::PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        let mut p = std::path::PathBuf::from(xdg);
        p.push("packlens");
        return p;
    }
    if let Ok(home) = std::env::var("HOME") {
        let mut p = std::path::PathBuf::from(home);
        p.push(".config/packlens");
        return p;
    }
    std::path::PathBuf::from("/tmp/packlens")
}
