use crate::models::{
    AdapterResult, CacheInfo, CleanupSuggestion, GlobalPackageInfo, Package, RuntimeInfo,
    VersionManagerInfo,
};

pub trait PackageAdapter: Send + Sync {
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;
    fn list_packages(&self) -> impl std::future::Future<Output = AdapterResult<Package>> + Send;
}

pub trait EnvironmentAdapter: Send + Sync {
    fn name(&self) -> &str;
    fn detect_runtimes(&self) -> impl std::future::Future<Output = Vec<RuntimeInfo>> + Send;
    fn detect_version_managers(
        &self,
    ) -> impl std::future::Future<Output = Vec<VersionManagerInfo>> + Send;
    fn list_global_packages(
        &self,
    ) -> impl std::future::Future<Output = Vec<GlobalPackageInfo>> + Send;
}

pub trait CacheAdapter: Send + Sync {
    fn name(&self) -> &str;
    fn list_caches(&self) -> impl std::future::Future<Output = Vec<CacheInfo>> + Send;
    fn suggest_cleanups(&self) -> impl std::future::Future<Output = Vec<CleanupSuggestion>> + Send;
}

pub mod apt;
pub mod cache;
pub mod desktop_file;
pub mod dev_cli;
pub mod flatpak;
pub mod java_env;
pub mod node_env;
pub mod python_env;
pub mod rust_env;
pub mod snap;
pub mod util;
