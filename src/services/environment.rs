use crate::adapters::java_env::JavaEnvAdapter;
use crate::adapters::node_env::NodeEnvAdapter;
use crate::adapters::python_env::PythonEnvAdapter;
use crate::adapters::rust_env::RustEnvAdapter;
use crate::adapters::EnvironmentAdapter;
use crate::models::{GlobalPackageInfo, RuntimeInfo, VersionManagerInfo};

pub struct EnvEvent {
    pub language: String,
    pub runtimes: Vec<RuntimeInfo>,
    pub version_managers: Vec<VersionManagerInfo>,
    pub global_packages: Vec<GlobalPackageInfo>,
}

pub async fn scan_all(
    tx: async_channel::Sender<EnvEvent>,
    token: tokio_util::sync::CancellationToken,
) {
    let adapters: Vec<Box<dyn EnvAdapterBoxed>> = vec![
        Box::new(PythonEnvAdapter),
        Box::new(NodeEnvAdapter),
        Box::new(RustEnvAdapter),
        Box::new(JavaEnvAdapter),
    ];

    for adapter in &adapters {
        if token.is_cancelled() {
            return;
        }

        tracing::info!("scanning env: {}", adapter.name());

        let runtimes = adapter.detect_runtimes_boxed().await;
        if token.is_cancelled() {
            return;
        }

        let version_managers = adapter.detect_version_managers_boxed().await;
        if token.is_cancelled() {
            return;
        }

        let global_packages = adapter.list_global_packages_boxed().await;
        if token.is_cancelled() {
            return;
        }

        let event = EnvEvent {
            language: adapter.name().to_string(),
            runtimes,
            version_managers,
            global_packages,
        };

        if tx.send(event).await.is_err() {
            return;
        }
    }
}

trait EnvAdapterBoxed: Send + Sync {
    fn name(&self) -> &str;
    fn detect_runtimes_boxed(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<RuntimeInfo>> + Send + '_>>;
    fn detect_version_managers_boxed(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<VersionManagerInfo>> + Send + '_>>;
    fn list_global_packages_boxed(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<GlobalPackageInfo>> + Send + '_>>;
}

impl<T: EnvironmentAdapter> EnvAdapterBoxed for T {
    fn name(&self) -> &str {
        EnvironmentAdapter::name(self)
    }
    fn detect_runtimes_boxed(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<RuntimeInfo>> + Send + '_>> {
        Box::pin(self.detect_runtimes())
    }
    fn detect_version_managers_boxed(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<VersionManagerInfo>> + Send + '_>>
    {
        Box::pin(self.detect_version_managers())
    }
    fn list_global_packages_boxed(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<GlobalPackageInfo>> + Send + '_>>
    {
        Box::pin(self.list_global_packages())
    }
}
