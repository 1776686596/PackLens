mod adapters;
mod app;
mod config;
mod error;
mod i18n;
mod models;
mod runtime;
mod services;
mod subprocess;
mod ui;
mod window;

use gtk::prelude::*;

fn main() -> gtk::glib::ExitCode {
    init_logging();
    tracing::info!("starting packlens");

    let _ = &*runtime::RUNTIME;

    let app = app::Application::new();
    let exit = app.run();

    exit
}

fn init_logging() {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let debug = std::env::args().any(|a| a == "--debug");
    let level = if debug { "debug" } else { "info" };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));

    let stderr_layer = fmt::layer().with_target(true);

    let log_dir = log_dir();
    let _ = std::fs::create_dir_all(&log_dir);
    let file_appender = tracing_appender::rolling::daily(&log_dir, "packlens.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true);

    std::mem::forget(guard);

    tracing_subscriber::registry()
        .with(filter)
        .with(stderr_layer)
        .with(file_layer)
        .init();
}

fn log_dir() -> std::path::PathBuf {
    if let Ok(state) = std::env::var("XDG_STATE_HOME") {
        let mut p = std::path::PathBuf::from(state);
        p.push("packlens");
        return p;
    }
    if let Ok(home) = std::env::var("HOME") {
        let mut p = std::path::PathBuf::from(home);
        p.push(".local/state/packlens");
        return p;
    }
    std::path::PathBuf::from("/tmp/packlens/logs")
}
