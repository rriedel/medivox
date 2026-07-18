//! Konfiguration/Defaults (unveraendert uebernommen von ../shim-rust/src/config.rs).

use std::env;

/// Ziel-Samplerate der Engine: 16 kHz mono float32.
pub const SAMPLE_RATE: u32 = 16_000;

pub const REQUEST_TIMEOUT_SECS: u64 = 60;
pub const DEFAULT_PSEUDO_STREAMING_ENABLED: bool = true;
pub const DEFAULT_STREAM_TICK_MS: u64 = 1200;
pub const DEFAULT_STREAM_WINDOW_MS: u64 = 12_000;
pub const DEFAULT_STREAM_RING_BUFFER_MS: u64 = 20_000;
pub const DEFAULT_STREAM_MIN_TRANSCRIBE_MS: u64 = 3_000;
pub const DEFAULT_STREAM_STABLE_PASSES: usize = 2;
pub const DEFAULT_STREAM_HOLDBACK_TOKENS: usize = 4;
pub const DEFAULT_STREAM_PREVIEW_ENABLED: bool = true;

/// Standard-Hotkey zum Umschalten: Control+Alt(Option)+Leertaste. Ueberschreibbar per
/// MEDIVOX_HOTKEY (Syntax des global-hotkey-Crates, z. B. "Super+Shift+D"). Auf macOS
/// registriert global-hotkey den Hotkey via Carbon RegisterEventHotKey -- dafuer ist
/// keine Bedienungshilfen-Berechtigung noetig (anders als fuer die Texteingabe).
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

pub fn stream_tick_ms() -> u64 {
    env::var("MEDIVOX_STREAM_TICK_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(|v: u64| v.clamp(200, 5_000))
        .unwrap_or(DEFAULT_STREAM_TICK_MS)
}

pub fn stream_window_ms() -> u64 {
    env::var("MEDIVOX_STREAM_WINDOW_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(|v: u64| v.clamp(2_000, 30_000))
        .unwrap_or(DEFAULT_STREAM_WINDOW_MS)
}

pub fn stream_ring_buffer_ms() -> u64 {
    env::var("MEDIVOX_STREAM_RING_BUFFER_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(|v: u64| v.clamp(5_000, 60_000))
        .unwrap_or(DEFAULT_STREAM_RING_BUFFER_MS)
}

pub fn stream_min_transcribe_ms() -> u64 {
    env::var("MEDIVOX_STREAM_MIN_TRANSCRIBE_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(|v: u64| v.clamp(500, 20_000))
        .unwrap_or(DEFAULT_STREAM_MIN_TRANSCRIBE_MS)
}

pub fn stream_stable_passes() -> usize {
    env::var("MEDIVOX_STREAM_STABLE_PASSES")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(|v: usize| v.clamp(1, 6))
        .unwrap_or(DEFAULT_STREAM_STABLE_PASSES)
}

pub fn stream_holdback_tokens() -> usize {
    env::var("MEDIVOX_STREAM_HOLDBACK_TOKENS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(|v: usize| v.clamp(0, 20))
        .unwrap_or(DEFAULT_STREAM_HOLDBACK_TOKENS)
}

pub fn stream_preview_enabled() -> bool {
    env::var("MEDIVOX_STREAM_PREVIEW_ENABLED")
        .ok()
        .map(|v| match v.trim().to_lowercase().as_str() {
            "0" | "false" | "off" | "no" => false,
            "1" | "true" | "on" | "yes" => true,
            _ => DEFAULT_STREAM_PREVIEW_ENABLED,
        })
        .unwrap_or(DEFAULT_STREAM_PREVIEW_ENABLED)
}

pub fn log_level() -> String {
    env::var("MEDIVOX_LOG_LEVEL").unwrap_or_else(|_| "info".to_string())
}
