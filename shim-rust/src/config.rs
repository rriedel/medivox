//! Konfiguration/Defaults (.NET-Pendant: ShimConfig.cs).

use std::env;

/// Ziel-Samplerate der Engine: 16 kHz mono float32.
pub const SAMPLE_RATE: u32 = 16_000;

pub const REQUEST_TIMEOUT_SECS: u64 = 60;
pub const DEFAULT_PSEUDO_STREAMING_ENABLED: bool = false;
pub const DEFAULT_STREAM_CHUNK_MS: u64 = 800;
pub const DEFAULT_STREAM_OVERLAP_MS: u64 = 250;
pub const DEFAULT_STREAM_MIN_AUDIO_MS: u64 = 2400;

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

pub fn pseudo_streaming_enabled() -> bool {
    env::var("MEDIVOX_PSEUDO_STREAMING")
        .ok()
        .map(|v| match v.trim().to_lowercase().as_str() {
            "0" | "false" | "off" | "no" => false,
            "1" | "true" | "on" | "yes" => true,
            _ => DEFAULT_PSEUDO_STREAMING_ENABLED,
        })
        .unwrap_or(DEFAULT_PSEUDO_STREAMING_ENABLED)
}

pub fn stream_chunk_ms() -> u64 {
    env::var("MEDIVOX_STREAM_CHUNK_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(|v: u64| v.clamp(200, 5_000))
        .unwrap_or(DEFAULT_STREAM_CHUNK_MS)
}

pub fn stream_overlap_ms() -> u64 {
    env::var("MEDIVOX_STREAM_OVERLAP_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(|v: u64| v.clamp(0, 2_000))
        .unwrap_or(DEFAULT_STREAM_OVERLAP_MS)
}

pub fn stream_min_audio_ms() -> u64 {
    env::var("MEDIVOX_STREAM_MIN_AUDIO_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(|v: u64| v.clamp(400, 10_000))
        .unwrap_or(DEFAULT_STREAM_MIN_AUDIO_MS)
}

pub fn log_level() -> String {
    env::var("MEDIVOX_LOG_LEVEL").unwrap_or_else(|_| "info".to_string())
}
