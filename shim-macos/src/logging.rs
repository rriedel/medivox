//! Logging-Setup (angelehnt an ../shim-rust/src/logging.rs, hier mit macOS-Pfaden).
//!
//! Schreibt taeglich rollierend nach ~/Library/Logs/Medivox -- dem uebliche Ort fuer
//! Anwendungslogs auf macOS. Eigener Dateipraefix (`shim-mac`), damit sich die Logs
//! nicht mit denen der anderen Shims mischen.

use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;

/// Der zurueckgegebene Guard muss bis zum Programmende leben -- er flusht den
/// Hintergrund-Writer beim Drop.
pub fn configure() -> anyhow::Result<WorkerGuard> {
    let dir = log_dir();
    std::fs::create_dir_all(&dir)?;

    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("shim-mac")
        .filename_suffix("log")
        .max_log_files(14)
        .build(&dir)?;
    let (writer, guard) = tracing_appender::non_blocking(appender);

    let filter = EnvFilter::try_new(config_level()).unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .with_target(true)
        .init();

    Ok(guard)
}

fn config_level() -> String {
    crate::config::log_level().to_lowercase()
}

pub fn log_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join("Library")
        .join("Logs")
        .join("Medivox")
}
