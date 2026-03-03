#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("config: {0}")]
    Config(#[from] ConfigError),
    #[error("adapter: {0}")]
    Adapter(#[from] AdapterError),
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("invalid value: {field} = {value}")]
    Validation { field: String, value: String },
}

#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("command failed: {cmd} (exit={code})")]
    CommandFailed { cmd: String, code: i32 },
    #[error("command timed out: {cmd} ({timeout_secs}s)")]
    Timeout { cmd: String, timeout_secs: u64 },
    #[error("parse error: {context}: {detail}")]
    Parse { context: String, detail: String },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
