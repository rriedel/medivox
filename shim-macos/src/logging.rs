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
        .with_thread_ids(false)
        .with_level(true)
        .init();

    Ok(guard)
}

fn config_level() -> String {
    // Lies RUST_LOG. Wenn nicht gesetzt, verwende Smart-Default:
    // - Bei debug/trace: Nur medivox-Crates auf dieser Ebene, alles andere auf warn
    // - Bei info/warn: Standard-Verhalten
    if let Ok(env_log) = std::env::var("RUST_LOG") {
        return env_log;
    }
    
    let level = crate::config::log_level().to_lowercase();
    match level.as_str() {
        "debug" => {
            // Nur der Shim selbst auf debug, externe Crates auf warn/error
            "medivox_shim=debug,tao=warn,ureq=warn,objc2=warn,cpal=warn,info".to_string()
        }
        "trace" => {
            // Trace nur für medivox, alles andere debug
            "medivox_shim=trace,tao=debug,ureq=debug,objc2=debug,cpal=debug,info".to_string()
        }
        _ => "info".to_string(),
    }
}

pub fn log_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join("Library")
        .join("Logs")
        .join("Medivox")
}
