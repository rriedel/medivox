//! Logging-Setup (.NET-Pendant: Logging.cs, dort Serilog).
//!
//! Schreibt taeglich rollierend nach %LocalAppData%\Medivox\logs. Eigener Dateipraefix
//! (`shim-rs`), damit sich die Logs nicht mit denen des .NET-Shims (`shim-`) mischen.

use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;

/// Der zurueckgegebene Guard muss bis zum Programmende leben -- er flusht den
/// Hintergrund-Writer beim Drop (Serilog-Pendant: Log.CloseAndFlush()).
pub fn configure() -> anyhow::Result<WorkerGuard> {
    let dir = log_dir();
    std::fs::create_dir_all(&dir)?;

    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("shim-rs")
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
    let local = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(local).join("Medivox").join("logs")
}
