use std::future::Future;

use crate::adapters::util::command_exists;
use crate::adapters::CacheAdapter;
use crate::models::{CacheInfo, CleanupSuggestion, RiskLevel};

pub struct AptCacheAdapter;

impl CacheAdapter for AptCacheAdapter {
    fn name(&self) -> &str {
        "apt"
    }

    fn list_caches(&self) -> impl Future<Output = Vec<CacheInfo>> + Send {
        async move {
            let path = "/var/cache/apt/archives";
            if !std::path::Path::new(path).exists() {
                return Vec::new();
            }
            let size = dir_size(path);
            vec![CacheInfo {
                name: "APT package cache".into(),
                path: path.into(),
                size,
                requires_sudo: true,
            }]
        }
    }

    fn suggest_cleanups(&self) -> impl Future<Output = Vec<CleanupSuggestion>> + Send {
        async move {
            if !command_exists("apt") {
                return Vec::new();
            }
            let path = "/var/cache/apt/archives";
            if !std::path::Path::new(path).exists() {
                return Vec::new();
            }
            let size = dir_size(path);
            if size == 0 {
                return Vec::new();
            }
            vec![CleanupSuggestion::new(
                "Clean APT package cache".into(),
                size,
                "apt clean".into(),
                true,
                RiskLevel::Safe,
            )]
            .into_iter()
            .flatten()
            .collect()
        }
    }
}

fn dir_size(path: &str) -> u64 {
    walkdir::WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}
