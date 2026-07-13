//! Konfiguration/Defaults (.NET-Pendant: ShimConfig.cs).

use std::env;

/// Ziel-Samplerate der Engine: 16 kHz mono float32.
pub const SAMPLE_RATE: u32 = 16_000;

pub const REQUEST_TIMEOUT_SECS: u64 = 60;

/// Standard-Hotkey zum Umschalten: Strg+Alt+Leertaste. Ueberschreibbar per
/// MEDIVOX_HOTKEY (Syntax des global-hotkey-Crates, z. B. "Control+Shift+D") --
/// noetig, wenn der .NET-Shim parallel laeuft: RegisterHotKey ist systemweit exklusiv.
pub const DEFAULT_HOTKEY: &str = "Control+Alt+Space";

pub fn engine_host() -> String {
    env::var("MEDIVOX_ENGINE_HOST").unwrap_or_else(|_| "127.0.0.1".to_string())
}

pub fn engine_port() -> u16 {
    env::var("MEDIVOX_ENGINE_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8123)
}

pub fn hotkey() -> String {
    env::var("MEDIVOX_HOTKEY").unwrap_or_else(|_| DEFAULT_HOTKEY.to_string())
}

pub fn log_level() -> String {
    env::var("MEDIVOX_LOG_LEVEL").unwrap_or_else(|_| "info".to_string())
}
