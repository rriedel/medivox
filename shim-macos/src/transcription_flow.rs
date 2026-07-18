use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::client::EngineClient;
use crate::config::SAMPLE_RATE;
use crate::recorder::Recorder;

struct StreamingSession {
    chunk_interval: Duration,
    min_chunk_samples: usize,
    overlap_samples: usize,
    overlap_tail: Vec<f32>,
    pending_audio: Vec<f32>,
    next_tick: Instant,
    next_seq: u64,
    in_flight: Arc<AtomicUsize>,
    results: Arc<std::sync::Mutex<BTreeMap<u64, String>>>,
}

impl StreamingSession {
    fn new(chunk_interval: Duration, min_chunk_samples: usize, overlap_samples: usize) -> Self {
        Self {
            chunk_interval,
            min_chunk_samples,
            overlap_samples,
            overlap_tail: Vec::new(),
            pending_audio: Vec::new(),
            next_tick: Instant::now() + chunk_interval,
            next_seq: 0,
            in_flight: Arc::new(AtomicUsize::new(0)),
            results: Arc::new(std::sync::Mutex::new(BTreeMap::new())),
        }
    }
}

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
    session: Option<StreamingSession>,
}

impl TranscriptionFlow {
    pub fn new(cfg: FlowConfig) -> Self {
        Self { cfg, session: None }
    }

    pub fn config(&self) -> FlowConfig {
        self.cfg
    }

    pub fn next_wakeup(&self, recording: bool) -> Option<Instant> {
        if !recording || !self.cfg.pseudo_streaming_enabled {
            return None;
        }
        if let Some(active) = &self.session {
            Some(active.next_tick)
        } else {
            Some(Instant::now() + self.cfg.chunk_interval)
        }
    }

    pub fn on_recording_started(&mut self) {
        if self.cfg.pseudo_streaming_enabled {
            self.session = Some(StreamingSession::new(
                self.cfg.chunk_interval,
                self.cfg.min_chunk_samples,
                self.cfg.overlap_samples,
            ));
        } else {
            self.session = None;
        }
    }

    pub fn process_tick(&mut self, recorder: &mut Recorder, engine: &Arc<EngineClient>) {
        if !self.cfg.pseudo_streaming_enabled {
            return;
        }

        let Some(active) = self.session.as_mut() else {
            return;
        };
        let now = Instant::now();
        if now < active.next_tick {
            return;
        }
        active.next_tick = now + active.chunk_interval;

        let chunk = match recorder.drain_chunk() {
            Ok(chunk) => chunk,
            Err(err) => {
                tracing::warn!("Chunk konnte nicht gelesen werden: {err:#}");
                return;
            }
        };
        if chunk.is_empty() {
            return;
        }

        active.pending_audio.extend_from_slice(&chunk);
        if active.pending_audio.len() < active.min_chunk_samples {
            return;
        }

        let ready_audio = std::mem::take(&mut active.pending_audio);

        let mut chunk_for_transcription =
            Vec::with_capacity(active.overlap_tail.len() + ready_audio.len());
        chunk_for_transcription.extend_from_slice(&active.overlap_tail);
        chunk_for_transcription.extend_from_slice(&ready_audio);

        if active.overlap_samples == 0 {
            active.overlap_tail.clear();
        } else {
            let tail_len = active.overlap_samples.min(ready_audio.len());
            active.overlap_tail = ready_audio[ready_audio.len() - tail_len..].to_vec();
        }

        let seq = active.next_seq;
        active.next_seq += 1;
        spawn_chunk_transcription(
            Arc::clone(engine),
            chunk_for_transcription,
            seq,
            Arc::clone(&active.in_flight),
            Arc::clone(&active.results),
        );
    }

    pub fn spawn_stop_transcription(
        &mut self,
        tail_audio: Vec<f32>,
        engine: Arc<EngineClient>,
        inject_text: fn(&str) -> Result<()>,
    ) {
        let active_session = self.session.take();
        let pseudo_active = self.cfg.pseudo_streaming_enabled;

        std::thread::spawn(move || {
            if !pseudo_active {
                let start = Instant::now();
                let text = match engine.transcribe(&tail_audio) {
                    Ok(text) => {
                        log_transcribe_metrics("full", None, tail_audio.len(), start.elapsed(), text.len());
                        text
                    }
                    Err(err) => {
                        log_transcribe_error_metrics(
                            "full",
                            None,
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
                return;
            }

            let mut merged = String::new();
            let mut final_prefix: Vec<f32> = Vec::new();
            let mut pending_audio: Vec<f32> = Vec::new();

            if let Some(session) = active_session {
                final_prefix = session.overlap_tail;
                pending_audio = session.pending_audio;
                while session.in_flight.load(Ordering::Acquire) > 0 {
                    std::thread::sleep(Duration::from_millis(10));
                }

                let mut ordered = session.results.lock().unwrap();
                for text in ordered.values() {
                    merged.push_str(text);
                }
                ordered.clear();
            }

            if !pending_audio.is_empty() || !tail_audio.is_empty() {
                let mut final_audio =
                    Vec::with_capacity(final_prefix.len() + pending_audio.len() + tail_audio.len());
                final_audio.extend_from_slice(&final_prefix);
                final_audio.extend_from_slice(&pending_audio);
                final_audio.extend_from_slice(&tail_audio);

                let start = Instant::now();
                let tail_text = match engine.transcribe(&final_audio) {
                    Ok(text) => {
                        log_transcribe_metrics(
                            "final_tail",
                            None,
                            final_audio.len(),
                            start.elapsed(),
                            text.len(),
                        );
                        text
                    }
                    Err(err) => {
                        log_transcribe_error_metrics(
                            "final_tail",
                            None,
                            final_audio.len(),
                            start.elapsed(),
                            &format!("{err:#}"),
                        );
                        tracing::error!("Finale Transkription fehlgeschlagen: {err:#}");
                        return;
                    }
                };
                merged.push_str(&tail_text);
            }

            let text = merged.trim().to_string();
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

fn spawn_chunk_transcription(
    engine: Arc<EngineClient>,
    chunk: Vec<f32>,
    seq: u64,
    in_flight: Arc<AtomicUsize>,
    results: Arc<std::sync::Mutex<BTreeMap<u64, String>>>,
) {
    in_flight.fetch_add(1, Ordering::AcqRel);
    std::thread::spawn(move || {
        let start = Instant::now();
        let audio_samples = chunk.len();
        let transcribed = engine.transcribe(&chunk);
        if let Ok(text) = transcribed {
            log_transcribe_metrics("chunk", Some(seq), audio_samples, start.elapsed(), text.len());
            if !text.is_empty() {
                let mut guard = results.lock().unwrap();
                guard.insert(seq, text);
            }
        } else if let Err(err) = transcribed {
            log_transcribe_error_metrics(
                "chunk",
                Some(seq),
                audio_samples,
                start.elapsed(),
                &format!("{err:#}"),
            );
            tracing::warn!("Chunk-Transkription fehlgeschlagen (seq={seq}): {err:#}");
        }
        in_flight.fetch_sub(1, Ordering::AcqRel);
    });
}

fn log_transcribe_metrics(
    kind: &str,
    seq: Option<u64>,
    audio_samples: usize,
    elapsed: Duration,
    text_len: usize,
) {
    let audio_s = audio_samples as f64 / SAMPLE_RATE as f64;
    let elapsed_s = elapsed.as_secs_f64();
    let rtf = if audio_s > 0.0 { elapsed_s / audio_s } else { 0.0 };
    tracing::info!(
        kind = kind,
        seq = ?seq,
        audio_s = format_args!("{audio_s:.3}"),
        elapsed_s = format_args!("{elapsed_s:.3}"),
        rtf = format_args!("{rtf:.3}"),
        chars = text_len,
        "transcribe_metrics"
    );
}

fn log_transcribe_error_metrics(
    kind: &str,
    seq: Option<u64>,
    audio_samples: usize,
    elapsed: Duration,
    error: &str,
) {
    let audio_s = audio_samples as f64 / SAMPLE_RATE as f64;
    let elapsed_s = elapsed.as_secs_f64();
    let rtf = if audio_s > 0.0 { elapsed_s / audio_s } else { 0.0 };
    tracing::warn!(
        kind = kind,
        seq = ?seq,
        audio_s = format_args!("{audio_s:.3}"),
        elapsed_s = format_args!("{elapsed_s:.3}"),
        rtf = format_args!("{rtf:.3}"),
        error = error,
        "transcribe_metrics_error"
    );
}
