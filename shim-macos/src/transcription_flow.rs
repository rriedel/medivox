use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::client::EngineClient;
use crate::config::SAMPLE_RATE;
use crate::recorder::Recorder;

#[derive(Clone, Copy)]
pub struct FlowConfig {
    pub pseudo_streaming_enabled: bool,
    pub chunk_interval: Duration,
    pub window_samples: usize,
    pub ring_capacity_samples: usize,
    pub min_transcribe_samples: usize,
    pub stable_passes: usize,
    pub holdback_tokens: usize,
    pub preview_enabled: bool,
}

struct StabilizationState {
    committed_raw: Vec<String>,
    committed_norm: Vec<String>,
    hypotheses: VecDeque<Hypothesis>,
}

struct Hypothesis {
    raw: Vec<String>,
    norm: Vec<String>,
}

impl StabilizationState {
    fn new() -> Self {
        Self {
            committed_raw: Vec::new(),
            committed_norm: Vec::new(),
            hypotheses: VecDeque::new(),
        }
    }

    fn push_hypothesis(&mut self, text: &str, stable_passes: usize, holdback_tokens: usize) {
        let mut hyp = tokenize_words(text);
        if hyp.raw.is_empty() {
            return;
        }

        // Bereits finalisierte Tokens am Fensterrand nicht erneut verarbeiten.
        let committed_overlap = max_overlap(&self.committed_norm, &hyp.norm);
        if committed_overlap > 0 {
            hyp.raw.drain(..committed_overlap);
            hyp.norm.drain(..committed_overlap);
        }
        if hyp.raw.is_empty() {
            return;
        }

        self.hypotheses.push_back(hyp);
        while self.hypotheses.len() > stable_passes.max(1) {
            self.hypotheses.pop_front();
        }

        let stable_prefix_len = longest_common_prefix_len_all(&self.hypotheses);
        let commit_count = stable_prefix_len.saturating_sub(holdback_tokens);
        if commit_count == 0 {
            return;
        }

        if let Some(latest) = self.hypotheses.back() {
            self.committed_raw
                .extend(latest.raw.iter().take(commit_count).cloned());
            self.committed_norm
                .extend(latest.norm.iter().take(commit_count).cloned());
        }

        for hyp in &mut self.hypotheses {
            let n = commit_count.min(hyp.raw.len());
            hyp.raw.drain(..n);
            hyp.norm.drain(..n);
        }
    }

    fn preview_text(&self) -> String {
        let mut out = String::new();
        if !self.committed_raw.is_empty() {
            out.push_str(&self.committed_raw.join(" "));
        }
        if let Some(latest) = self.hypotheses.back() {
            if !latest.raw.is_empty() {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(&latest.raw.join(" "));
            }
        }
        out
    }

    fn finalize_text(&mut self) -> String {
        if let Some(latest) = self.hypotheses.back() {
            self.committed_raw.extend(latest.raw.clone());
            self.committed_norm.extend(latest.norm.clone());
        }
        self.hypotheses.clear();
        self.committed_raw.join(" ")
    }
}

pub struct TranscriptionFlow {
    cfg: FlowConfig,
    ring: VecDeque<f32>,
    next_tick: Instant,
    state: Arc<Mutex<StabilizationState>>,
    in_flight: Arc<AtomicBool>,
    seq: AtomicU64,
}

impl TranscriptionFlow {
    pub fn new(cfg: FlowConfig) -> Self {
        Self {
            cfg,
            ring: VecDeque::new(),
            next_tick: Instant::now() + cfg.chunk_interval,
            state: Arc::new(Mutex::new(StabilizationState::new())),
            in_flight: Arc::new(AtomicBool::new(false)),
            seq: AtomicU64::new(0),
        }
    }

    pub fn config(&self) -> FlowConfig {
        self.cfg
    }

    pub fn next_wakeup(&self, recording: bool) -> Option<Instant> {
        if !recording || !self.cfg.pseudo_streaming_enabled {
            return None;
        }
        Some(self.next_tick)
    }

    pub fn on_recording_started(&mut self) {
        self.ring.clear();
        self.next_tick = Instant::now() + self.cfg.chunk_interval;
        self.in_flight.store(false, Ordering::Release);
        self.seq.store(0, Ordering::Release);
        if let Ok(mut state) = self.state.lock() {
            *state = StabilizationState::new();
        }
    }

    pub fn process_tick(&mut self, recorder: &mut Recorder, engine: &Arc<EngineClient>) {
        if !self.cfg.pseudo_streaming_enabled {
            return;
        }

        let chunk = match recorder.drain_chunk() {
            Ok(chunk) => chunk,
            Err(err) => {
                tracing::warn!("Chunk konnte nicht gelesen werden: {err:#}");
                return;
            }
        };
        self.append_to_ring(&chunk);

        let now = Instant::now();
        if now < self.next_tick {
            return;
        }
        self.next_tick = now + self.cfg.chunk_interval;

        if self.in_flight.load(Ordering::Acquire) {
            tracing::debug!("Transkription laeuft noch, Tick wird uebersprungen");
            return;
        }

        let window = self.window_snapshot();
        if window.len() < self.cfg.min_transcribe_samples {
            return;
        }

        let seq = self.seq.fetch_add(1, Ordering::AcqRel);
        self.spawn_window_transcription(
            Arc::clone(engine),
            window,
            seq,
            "window",
        );
    }

    pub fn spawn_stop_transcription(
        &mut self,
        tail_audio: Vec<f32>,
        engine: Arc<EngineClient>,
        inject_text: fn(&str) -> Result<()>,
    ) {
        self.append_to_ring(&tail_audio);
        let final_window = self.window_snapshot();
        let cfg = self.cfg;
        let state = Arc::clone(&self.state);
        let in_flight = Arc::clone(&self.in_flight);

        std::thread::spawn(move || {
            while in_flight.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(10));
            }

            if !cfg.pseudo_streaming_enabled {
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

            if final_window.len() >= cfg.min_transcribe_samples {
                let start = Instant::now();
                match engine.transcribe(&final_window) {
                    Ok(text) => {
                        log_transcribe_metrics(
                            "final_window",
                            None,
                            final_window.len(),
                            start.elapsed(),
                            text.len(),
                        );
                        if let Ok(mut s) = state.lock() {
                            s.push_hypothesis(&text, cfg.stable_passes, cfg.holdback_tokens);
                        }
                    }
                    Err(err) => {
                        log_transcribe_error_metrics(
                            "final_window",
                            None,
                            final_window.len(),
                            start.elapsed(),
                            &format!("{err:#}"),
                        );
                    }
                }
            }

            let final_text = {
                let mut s = state.lock().unwrap();
                s.finalize_text()
            };

            tracing::info!("Transkriptionsergebnis: {final_text}");
            if final_text.trim().is_empty() {
                return;
            }
            if let Err(err) = inject_text(final_text.trim()) {
                tracing::error!("Texteingabe fehlgeschlagen: {err:#}");
            }
        });
    }

    fn append_to_ring(&mut self, chunk: &[f32]) {
        for &sample in chunk {
            self.ring.push_back(sample);
        }
        while self.ring.len() > self.cfg.ring_capacity_samples {
            self.ring.pop_front();
        }
    }

    fn window_snapshot(&self) -> Vec<f32> {
        let keep = self.cfg.window_samples.min(self.ring.len());
        let start = self.ring.len().saturating_sub(keep);
        self.ring.iter().skip(start).copied().collect()
    }

    fn spawn_window_transcription(
        &self,
        engine: Arc<EngineClient>,
        audio: Vec<f32>,
        seq: u64,
        kind: &'static str,
    ) {
        if self
            .in_flight
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }

        let state = Arc::clone(&self.state);
        let in_flight = Arc::clone(&self.in_flight);
        let cfg = self.cfg;
        std::thread::spawn(move || {
            let start = Instant::now();
            let result = engine.transcribe(&audio);
            match result {
                Ok(text) => {
                    log_transcribe_metrics(kind, Some(seq), audio.len(), start.elapsed(), text.len());
                    let preview = {
                        let mut s = state.lock().unwrap();
                        s.push_hypothesis(&text, cfg.stable_passes, cfg.holdback_tokens);
                        s.preview_text()
                    };
                    if cfg.preview_enabled {
                        let preview_compact = condense_whitespace(&preview);
                        tracing::info!(
                            seq = seq,
                            preview_chars = preview_compact.len(),
                            preview = truncate_preview(&preview_compact, 220),
                            "transcribe_preview"
                        );
                    }
                }
                Err(err) => {
                    log_transcribe_error_metrics(
                        kind,
                        Some(seq),
                        audio.len(),
                        start.elapsed(),
                        &format!("{err:#}"),
                    );
                    tracing::warn!("Fenster-Transkription fehlgeschlagen (seq={seq}): {err:#}");
                }
            }
            in_flight.store(false, Ordering::Release);
        });
    }
}

fn tokenize_words(text: &str) -> Hypothesis {
    let mut raw = Vec::new();
    let mut norm = Vec::new();
    for token in text.split_whitespace() {
        let normalized = normalize_token(token);
        if normalized.is_empty() {
            continue;
        }
        raw.push(token.to_string());
        norm.push(normalized);
    }
    Hypothesis { raw, norm }
}

fn normalize_token(token: &str) -> String {
    token
        .trim_matches(|c: char| !c.is_alphanumeric() && c != '\'' && c != '-')
        .to_lowercase()
}

fn max_overlap(committed: &[String], candidate: &[String]) -> usize {
    let max = committed.len().min(candidate.len());
    for overlap in (1..=max).rev() {
        if committed[committed.len() - overlap..] == candidate[..overlap] {
            return overlap;
        }
    }
    0
}

fn longest_common_prefix_len(a: &[String], b: &[String]) -> usize {
    let mut i = 0;
    let max = a.len().min(b.len());
    while i < max && a[i] == b[i] {
        i += 1;
    }
    i
}

fn longest_common_prefix_len_all(hypotheses: &VecDeque<Hypothesis>) -> usize {
    let Some(first) = hypotheses.front() else {
        return 0;
    };
    hypotheses
        .iter()
        .skip(1)
        .fold(first.norm.len(), |acc, hyp| {
            acc.min(longest_common_prefix_len(&first.norm, &hyp.norm))
        })
}

fn condense_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_preview(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars).collect();
    out.push_str("...");
    out
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
