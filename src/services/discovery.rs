use crate::adapters::apt::AptAdapter;
use crate::adapters::desktop_file::DesktopFileAdapter;
use crate::adapters::flatpak::FlatpakAdapter;
use crate::adapters::snap::SnapAdapter;
use crate::adapters::PackageAdapter;
use crate::models::Package;
use tokio::task::JoinSet;

fn default_adapters() -> Vec<Box<dyn PackageAdapterBoxed>> {
    vec![
        Box::new(AptAdapter),
        Box::new(SnapAdapter),
        Box::new(FlatpakAdapter),
        Box::new(DesktopFileAdapter),
    ]
}

pub async fn discover_all(
    tx: async_channel::Sender<DiscoveryEvent>,
    token: tokio_util::sync::CancellationToken,
) {
    let adapters = default_adapters();

    let mut tasks = JoinSet::new();

    for adapter in adapters {
        if token.is_cancelled() {
            tasks.abort_all();
            return;
        }
        if !adapter.is_available() {
            tracing::debug!("adapter {} not available, skipping", adapter.name());
            continue;
        }

        let name = adapter.name().to_string();
        tasks.spawn(async move {
            tracing::info!("running adapter: {name}");
            let result = adapter.list_packages_boxed().await;
            (name, result)
        });
    }

    while let Some(joined) = tasks.join_next().await {
        if token.is_cancelled() {
            tasks.abort_all();
            return;
        }

        let (source, result) = match joined {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("adapter task join failed: {e}");
                continue;
            }
        };

        if token.is_cancelled() {
            tasks.abort_all();
            return;
        }

        let warning_count = result.warnings.len();
        if warning_count > 0 {
            for w in &result.warnings {
                tracing::warn!("{}", w);
            }
        }

        let event = DiscoveryEvent {
            source,
            packages: result.items,
            warnings: result.warnings,
        };

        if tx.send(event).await.is_err() {
            return;
        }
    }
}

pub struct DiscoveryEvent {
    pub source: String,
    pub packages: Vec<Package>,
    pub warnings: Vec<String>,
}

// We need a trait-object-safe wrapper since PackageAdapter uses RPITIT
trait PackageAdapterBoxed: Send + Sync {
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;
    fn list_packages_boxed(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = crate::models::AdapterResult<Package>> + Send + '_>,
    >;
}

impl<T: PackageAdapter> PackageAdapterBoxed for T {
    fn name(&self) -> &str {
        PackageAdapter::name(self)
    }
    fn is_available(&self) -> bool {
        PackageAdapter::is_available(self)
    }
    fn list_packages_boxed(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = crate::models::AdapterResult<Package>> + Send + '_>,
    > {
        Box::pin(self.list_packages())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AdapterResult;
    use std::time::Instant;

    #[test]
    fn default_adapters_exclude_dev_cli() {
        let adapters = default_adapters();
        assert!(!adapters.iter().any(|a| a.name() == "dev-cli"));
    }

    struct SleepAdapter {
        name: &'static str,
        sleep_ms: u64,
    }

    impl PackageAdapterBoxed for SleepAdapter {
        fn name(&self) -> &str {
            self.name
        }

        fn is_available(&self) -> bool {
            true
        }

        fn list_packages_boxed(
            &self,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AdapterResult<Package>> + Send + '_>>
        {
            Box::pin(async move {
                tokio::time::sleep(std::time::Duration::from_millis(self.sleep_ms)).await;
                AdapterResult {
                    items: Vec::new(),
                    warnings: Vec::new(),
                    duration_ms: self.sleep_ms,
                    timestamp: 0.0,
                }
            })
        }
    }

    async fn discover_with_adapters(
        adapters: Vec<Box<dyn PackageAdapterBoxed>>,
    ) -> Vec<DiscoveryEvent> {
        let token = tokio_util::sync::CancellationToken::new();
        let (tx, rx) = async_channel::bounded::<DiscoveryEvent>(16);

        let mut tasks = JoinSet::new();
        for adapter in adapters {
            if !adapter.is_available() {
                continue;
            }
            let name = adapter.name().to_string();
            tasks.spawn(async move {
                let result = adapter.list_packages_boxed().await;
                (name, result)
            });
        }

        while let Some(joined) = tasks.join_next().await {
            if token.is_cancelled() {
                tasks.abort_all();
                break;
            }
            let Ok((source, result)) = joined else {
                continue;
            };
            let event = DiscoveryEvent {
                source,
                packages: result.items,
                warnings: result.warnings,
            };
            if tx.send(event).await.is_err() {
                break;
            }
        }
        drop(tx);

        let mut events = Vec::new();
        while let Ok(event) = rx.recv().await {
            events.push(event);
        }
        events
    }

    #[test]
    fn discovery_runs_adapters_concurrently() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_time()
            .build()
            .expect("create tokio runtime");

        runtime.block_on(async {
            let started = Instant::now();
            let events = discover_with_adapters(vec![
                Box::new(SleepAdapter {
                    name: "a",
                    sleep_ms: 250,
                }),
                Box::new(SleepAdapter {
                    name: "b",
                    sleep_ms: 250,
                }),
                Box::new(SleepAdapter {
                    name: "c",
                    sleep_ms: 250,
                }),
            ])
            .await;

            let elapsed = started.elapsed().as_millis() as u64;
            assert_eq!(events.len(), 3);
            assert!(
                elapsed < 600,
                "expected concurrent run, elapsed={elapsed}ms"
            );
        });
    }
}
