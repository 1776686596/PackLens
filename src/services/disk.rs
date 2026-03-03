use crate::adapters::cache::apt_cache::AptCacheAdapter;
use crate::adapters::cache::cargo_cache::CargoCacheAdapter;
use crate::adapters::cache::conda_cache::CondaCacheAdapter;
use crate::adapters::cache::docker_cache::DockerCacheAdapter;
use crate::adapters::cache::npm_cache::NpmCacheAdapter;
use crate::adapters::cache::pip_cache::PipCacheAdapter;
use crate::adapters::CacheAdapter;
use crate::models::{CacheInfo, Package};
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone)]
pub struct FolderUsage {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
}

pub struct DiskEvent {
    pub scan_id: u64,
    pub caches: Vec<CacheInfo>,
    pub roots: Vec<String>,
    pub folder_usage: HashMap<String, Vec<FolderUsage>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanMode {
    Fast,
    Full,
}

pub async fn scan_all(
    tx: async_channel::Sender<DiskEvent>,
    token: tokio_util::sync::CancellationToken,
    mode: ScanMode,
    scan_id: u64,
) {
    let adapters: Vec<Box<dyn CacheAdapterBoxed>> = vec![
        Box::new(AptCacheAdapter),
        Box::new(PipCacheAdapter),
        Box::new(NpmCacheAdapter),
        Box::new(CondaCacheAdapter),
        Box::new(CargoCacheAdapter),
        Box::new(DockerCacheAdapter),
    ];

    let mut all_caches = Vec::new();

    for adapter in &adapters {
        if token.is_cancelled() {
            return;
        }

        tracing::info!("scanning cache: {}", adapter.name());
        let caches = adapter.list_caches_boxed().await;
        if token.is_cancelled() {
            return;
        }

        all_caches.extend(caches);
    }

    let mut roots: Vec<String> = all_caches
        .iter()
        .map(|cache| normalize_path(&cache.path))
        .collect();
    roots.extend(system_scan_roots(mode));
    roots.sort();
    roots.dedup();

    let fast_roots: Vec<String> = roots
        .iter()
        .filter(|root| root.as_str() != "/")
        .cloned()
        .collect();

    let mut folder_usage: HashMap<String, Vec<FolderUsage>> = HashMap::new();
    for root in &fast_roots {
        if token.is_cancelled() {
            return;
        }

        tracing::info!("analyzing filesystem root: {}", root);
        let root_for_worker = root.clone();
        let analyzed = tokio::task::spawn_blocking(move || analyze_tree_entries(&root_for_worker))
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("disk analyzer worker failed: {e}");
                HashMap::new()
            });

        for (parent, mut children) in analyzed {
            folder_usage
                .entry(parent)
                .or_default()
                .append(&mut children);
        }
    }

    sort_and_dedup_children(&mut folder_usage);

    let event = DiskEvent {
        scan_id,
        caches: all_caches.clone(),
        roots: roots.clone(),
        folder_usage: folder_usage.clone(),
    };
    if tx.send(event).await.is_err() {
        return;
    }

    if mode == ScanMode::Full && roots.iter().any(|root| root == "/") {
        if token.is_cancelled() {
            return;
        }

        tracing::info!("analyzing filesystem root: /");
        let analyzed = tokio::task::spawn_blocking(|| analyze_tree_entries("/"))
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("disk analyzer worker failed: {e}");
                HashMap::new()
            });

        for (parent, mut children) in analyzed {
            folder_usage
                .entry(parent)
                .or_default()
                .append(&mut children);
        }

        sort_and_dedup_children(&mut folder_usage);

        let final_event = DiskEvent {
            scan_id,
            caches: all_caches,
            roots,
            folder_usage,
        };
        let _ = tx.send(final_event).await;
    }
}

pub fn rank_packages(packages: &[Package], top_n: u32) -> Vec<&Package> {
    let top_n = top_n.clamp(10, 200) as usize;
    let mut sorted: Vec<&Package> = packages.iter().collect();
    sorted.sort_by(|a, b| match (a.size, b.size) {
        (Some(a_s), Some(b_s)) => b_s.cmp(&a_s),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    sorted.truncate(top_n);
    sorted
}

trait CacheAdapterBoxed: Send + Sync {
    fn name(&self) -> &str;
    fn list_caches_boxed(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<CacheInfo>> + Send + '_>>;
}

impl<T: CacheAdapter> CacheAdapterBoxed for T {
    fn name(&self) -> &str {
        CacheAdapter::name(self)
    }
    fn list_caches_boxed(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<CacheInfo>> + Send + '_>> {
        Box::pin(self.list_caches())
    }
}

pub fn analyze_tree_entries(root: &str) -> HashMap<String, Vec<FolderUsage>> {
    let root = normalize_path(root);
    let root_path = Path::new(&root);
    if !root_path.exists() || !root_path.is_dir() {
        return HashMap::new();
    }

    let mut dir_sizes: HashMap<String, u64> = HashMap::new();
    let mut files_by_parent: HashMap<String, Vec<FolderUsage>> = HashMap::new();

    dir_sizes.insert(root.clone(), 0);

    for entry in walkdir::WalkDir::new(root_path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if should_skip_path(&root, path) {
            continue;
        }

        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }

        let size = metadata.len();
        let file_path = normalize_path(path.to_string_lossy().as_ref());
        let parent = path
            .parent()
            .and_then(|v| v.to_str())
            .map(normalize_path)
            .unwrap_or_else(|| root.clone());
        files_by_parent
            .entry(parent)
            .or_default()
            .push(FolderUsage {
                name: display_name(&file_path),
                path: file_path,
                size,
                is_dir: false,
            });

        let mut parent = path.parent();
        while let Some(dir) = parent {
            if !dir.starts_with(root_path) {
                break;
            }
            let dir_key = normalize_path(dir.to_string_lossy().as_ref());
            let current = dir_sizes.entry(dir_key).or_insert(0);
            *current = current.saturating_add(size);
            if dir == root_path {
                break;
            }
            parent = dir.parent();
        }
    }

    let mut children_by_parent: HashMap<String, Vec<FolderUsage>> = HashMap::new();

    for (dir_path, size) in &dir_sizes {
        if dir_path == &root {
            continue;
        }

        let Some(parent) = Path::new(dir_path)
            .parent()
            .and_then(|v| v.to_str())
            .map(normalize_path)
        else {
            continue;
        };

        children_by_parent
            .entry(parent)
            .or_default()
            .push(FolderUsage {
                name: display_name(dir_path),
                path: dir_path.clone(),
                size: *size,
                is_dir: true,
            });
    }

    for (parent, mut files) in files_by_parent {
        children_by_parent
            .entry(parent)
            .or_default()
            .append(&mut files);
    }

    children_by_parent.entry(root).or_default();

    children_by_parent
}

fn normalize_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        trimmed.to_string()
    }
}

fn display_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|v| v.to_str())
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| path.to_string())
}

fn system_scan_roots(mode: ScanMode) -> Vec<String> {
    let mut roots = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        roots.push(normalize_path(&home));
    }
    if mode == ScanMode::Full {
        roots.push("/".to_string());
    }
    roots
}

fn should_skip_path(scan_root: &str, path: &Path) -> bool {
    if scan_root != "/" {
        return false;
    }

    let raw = path.to_string_lossy();
    let blocked = ["/proc", "/sys", "/dev", "/run", "/tmp"];
    blocked
        .iter()
        .any(|prefix| raw == *prefix || raw.starts_with(&format!("{prefix}/")))
}

fn sort_and_dedup_children(folder_usage: &mut HashMap<String, Vec<FolderUsage>>) {
    for children in folder_usage.values_mut() {
        children.sort_by(|a, b| {
            b.size
                .cmp(&a.size)
                .then_with(|| b.is_dir.cmp(&a.is_dir))
                .then_with(|| a.path.cmp(&b.path))
        });
        children.dedup_by(|a, b| a.path == b.path && a.is_dir == b.is_dir);
    }
}
