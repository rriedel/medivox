use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::client::EngineClient;
use crate::config::SAMPLE_RATE;
use crate::recorder::Recorder;

#[derive(Clone, Copy)]
pub struct FlowConfig {
    pub pseudo_streaming_enabled: bool,
    pub chunk_interval: Duration,
    pub min_chunk_samples: usize,
    pub min_audio_ms: u64,
    pub overlap_ms: u64,
    pub overlap_samples: usize,
}

pub struct TranscriptionFlow {
    cfg: FlowConfig,
    warned_not_supported: bool,
}

impl TranscriptionFlow {
    pub fn new(cfg: FlowConfig) -> Self {
        Self {
            cfg,
            warned_not_supported: false,
        }
    }

    pub fn config(&self) -> FlowConfig {
        self.cfg
    }

    pub fn next_wakeup(&self, _recording: bool) -> Option<Instant> {
        None
    }

    pub fn on_recording_started(&mut self) {
        // Platzhalter fuer den spaeteren Port der Streaming-Session-Initialisierung.
    }

    pub fn process_tick(&mut self, _recorder: &mut Recorder, _engine: &Arc<EngineClient>) {
        // Auf Windows bleibt das Verhalten vorerst unveraendert (Full-Utterance).
        if self.cfg.pseudo_streaming_enabled && !self.warned_not_supported {
            self.warned_not_supported = true;
            tracing::warn!(
                "Pseudo-Streaming ist im Windows-Rust-Shim noch nicht umgesetzt und wird ignoriert"
            );
        }
    }

    pub fn spawn_stop_transcription(
        &mut self,
        tail_audio: Vec<f32>,
        engine: Arc<EngineClient>,
        inject_text: fn(&str) -> Result<()>,
    ) {
        std::thread::spawn(move || {
            let start = Instant::now();
            let text = match engine.transcribe(&tail_audio) {
                Ok(text) => {
                    log_transcribe_metrics("full", tail_audio.len(), start.elapsed(), text.len());
                    text
                }
                Err(err) => {
                    log_transcribe_error_metrics(
                        "full",
                        tail_audio.len(),
                        start.elapsed(),
                        &format!("{err:#}"),
                    );
                    tracing::error!("Transkription fehlgeschlagen: {err:#}");
                    return;
                }
            };

            let text = text.trim().to_string();
            tracing::info!("Transkriptionsergebnis: {text}");
            if text.is_empty() {
                return;
            }
            if let Err(err) = inject_text(&text) {
                tracing::error!("Texteingabe fehlgeschlagen: {err:#}");
            }
        });
    }
}

fn log_transcribe_metrics(kind: &str, audio_samples: usize, elapsed: Duration, text_len: usize) {
    let audio_s = audio_samples as f64 / SAMPLE_RATE as f64;
    let elapsed_s = elapsed.as_secs_f64();
    let rtf = if audio_s > 0.0 { elapsed_s / audio_s } else { 0.0 };
    tracing::info!(
        kind = kind,
        audio_s = format_args!("{audio_s:.3}"),
        elapsed_s = format_args!("{elapsed_s:.3}"),
        rtf = format_args!("{rtf:.3}"),
        chars = text_len,
        "transcribe_metrics"
    );
}

fn log_transcribe_error_metrics(kind: &str, audio_samples: usize, elapsed: Duration, error: &str) {
    let audio_s = audio_samples as f64 / SAMPLE_RATE as f64;
    let elapsed_s = elapsed.as_secs_f64();
    let rtf = if audio_s > 0.0 { elapsed_s / audio_s } else { 0.0 };
    tracing::warn!(
        kind = kind,
        audio_s = format_args!("{audio_s:.3}"),
        elapsed_s = format_args!("{elapsed_s:.3}"),
        rtf = format_args!("{rtf:.3}"),
        error = error,
        "transcribe_metrics_error"
    );
}
