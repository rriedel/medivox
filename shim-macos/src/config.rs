//! Konfiguration/Defaults (unveraendert uebernommen von ../shim-rust/src/config.rs).

use std::env;

/// Ziel-Samplerate der Engine: 16 kHz mono float32.
pub const SAMPLE_RATE: u32 = 16_000;

pub const REQUEST_TIMEOUT_SECS: u64 = 60;

/// Schaltet den Streaming-Pfad fuer die Engine-Ansteuerung ein/aus.
///
/// Wirkung:
/// - true: Ringpuffer + periodische Fenster-Transkription + Stabilisierung.
/// - false: klassische Full-Utterance-Transkription erst beim Stop.
pub const DEFAULT_PSEUDO_STREAMING_ENABLED: bool = true;

/// Intervall der Streaming-Ticks (in ms).
///
/// Wirkung:
/// - kleiner: haeufigere Updates, schnellere Preview, aber mehr CPU/Engine-Last.
/// - groesser: weniger Last, aber traegere Aktualisierung.
pub const DEFAULT_STREAM_TICK_MS: u64 = 1200;

/// Groesse des Rueckblick-Fensters fuer jeden Re-Decode (in ms).
///
/// Wirkung:
/// - kleiner: schneller pro Request, aber weniger Kontext fuer Stabilisierung.
/// - groesser: robuster bei Wortgrenzen/Korrekturen, aber teuerer pro Request.
pub const DEFAULT_STREAM_WINDOW_MS: u64 = 12_000;

/// Maximale Ringpuffer-Laenge (in ms).
///
/// Wirkung:
/// - muss >= STREAM_WINDOW_MS sein, sonst wird Kontext vorzeitig abgeschnitten.
/// - groesser: mehr Historie fuer spaetere Fenster, aber mehr Speicher.
pub const DEFAULT_STREAM_RING_BUFFER_MS: u64 = 20_000;

/// Mindest-Audiolaenge fuer einen Streaming-Decode (in ms).
///
/// Wirkung:
/// - verhindert, dass sehr kurze Fenster nur fixe Engine-Overheads triggern.
/// - groesser: bessere Effizienz, aber spaeterer erster Preview-Text.
pub const DEFAULT_STREAM_MIN_TRANSCRIBE_MS: u64 = 3_000;

/// Anzahl aufeinanderfolgender Hypothesen, die fuer ein stabiles Praefix
/// uebereinstimmen muessen.
///
/// Wirkung:
/// - groesser: stabiler/konservativer Commit, aber mehr Verzoegerung.
/// - kleiner: reaktiver, aber eher flackernde Teilresultate.
pub const DEFAULT_STREAM_STABLE_PASSES: usize = 2;

/// Anzahl Tokens am Ende eines stabilen Praefixes, die absichtlich noch NICHT
/// final committed werden.
///
/// Wirkung:
/// - groesser: weniger Grenzfehler, aber mehr Text bleibt vorlaeufig.
/// - kleiner: mehr sofortiger Output, aber hoehere Korrekturgefahr.
pub const DEFAULT_STREAM_HOLDBACK_TOKENS: usize = 4;

/// Schaltet laufende Preview-Logs waehrend der Aufnahme ein/aus.
///
/// Wirkung:
/// - true: laufendes Monitoring/Debugging moeglich.
/// - false: weniger Log-Rauschen.
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

/// Liest MEDIVOX_PSEUDO_STREAMING.
/// Akzeptiert: 1/0, true/false, on/off, yes/no.
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

/// Liest MEDIVOX_STREAM_TICK_MS.
/// Clamp: 200..5000 ms.
pub fn stream_tick_ms() -> u64 {
    env::var("MEDIVOX_STREAM_TICK_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(|v: u64| v.clamp(200, 5_000))
        .unwrap_or(DEFAULT_STREAM_TICK_MS)
}

/// Liest MEDIVOX_STREAM_WINDOW_MS.
/// Clamp: 2000..30000 ms.
pub fn stream_window_ms() -> u64 {
    env::var("MEDIVOX_STREAM_WINDOW_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(|v: u64| v.clamp(2_000, 30_000))
        .unwrap_or(DEFAULT_STREAM_WINDOW_MS)
}

/// Liest MEDIVOX_STREAM_RING_BUFFER_MS.
/// Clamp: 5000..60000 ms.
pub fn stream_ring_buffer_ms() -> u64 {
    env::var("MEDIVOX_STREAM_RING_BUFFER_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(|v: u64| v.clamp(5_000, 60_000))
        .unwrap_or(DEFAULT_STREAM_RING_BUFFER_MS)
}

/// Liest MEDIVOX_STREAM_MIN_TRANSCRIBE_MS.
/// Clamp: 500..20000 ms.
pub fn stream_min_transcribe_ms() -> u64 {
    env::var("MEDIVOX_STREAM_MIN_TRANSCRIBE_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(|v: u64| v.clamp(500, 20_000))
        .unwrap_or(DEFAULT_STREAM_MIN_TRANSCRIBE_MS)
}

/// Liest MEDIVOX_STREAM_STABLE_PASSES.
/// Clamp: 1..6.
pub fn stream_stable_passes() -> usize {
    env::var("MEDIVOX_STREAM_STABLE_PASSES")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(|v: usize| v.clamp(1, 6))
        .unwrap_or(DEFAULT_STREAM_STABLE_PASSES)
}

/// Liest MEDIVOX_STREAM_HOLDBACK_TOKENS.
/// Clamp: 0..20.
pub fn stream_holdback_tokens() -> usize {
    env::var("MEDIVOX_STREAM_HOLDBACK_TOKENS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(|v: usize| v.clamp(0, 20))
        .unwrap_or(DEFAULT_STREAM_HOLDBACK_TOKENS)
}

/// Liest MEDIVOX_STREAM_PREVIEW_ENABLED.
/// Akzeptiert: 1/0, true/false, on/off, yes/no.
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

/// Liest MEDIVOX_SAVE_AUDIO.
/// Akzeptiert: 1/0, true/false, on/off, yes/no.
/// Wenn true, wird das aufgenommene Audio beim Stop als WAV gespeichert.
pub fn save_audio() -> bool {
    env::var("MEDIVOX_SAVE_AUDIO")
        .ok()
        .map(|v| match v.trim().to_lowercase().as_str() {
            "0" | "false" | "off" | "no" => false,
            "1" | "true" | "on" | "yes" => true,
            _ => false,
        })
        .unwrap_or(false)
}
