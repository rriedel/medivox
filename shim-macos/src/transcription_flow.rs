use std::collections::VecDeque;
use std::path::PathBuf;
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

        // Bereits finalisierte Tokens finden und herausschneiden.
        // WICHTIG: Beim Sliding Window beginnt die neue Hypothesis VOR der committed-
        // Frontier (das Fenster startet tick_ms früher als wo committed aufhört).
        // Deshalb muss die committed-Grenze IRGENDWO IM INNEREN der Hypothesis
        // gesucht werden – nicht nur am Anfang.
        let skip = find_committed_frontier(&self.committed_norm, &hyp.norm);
        if skip > 0 {
            tracing::debug!(
                committed_len = self.committed_norm.len(),
                hyp_len = hyp.norm.len(),
                skip,
                "committed_frontier_found"
            );
            hyp.raw.drain(..skip);
            hyp.norm.drain(..skip);
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
    save_audio: bool,
    /// Optional: Kompletter Audio-Stream während der Aufnahme (für reproduzierbare Tests)
    audio_history: Vec<f32>,
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
            save_audio: crate::config::save_audio(),
            audio_history: Vec::new(),
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
        self.audio_history.clear();
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
        // Speichere Audio, wenn konfiguriert
        if let Err(err) = self.save_ring_to_wav() {
            tracing::warn!("Audio konnte nicht gespeichert werden: {err:#}");
        }

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
            if self.save_audio {
                self.audio_history.push(sample);
            }
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

    /// Lädt ein WAV-File und speist es mit Verzögerung (Echtzeit-Geschwindigkeit)
    /// in den Ringpuffer ein. Für reproduzierbare Tests des Shim-Ablaufs.
    ///
    /// Dies ist hauptsächlich für Testzwecke gedacht, nicht für die normale Nutzung.
    pub fn load_and_feed_wav(&mut self, wav_path: &std::path::Path) -> Result<()> {
        use std::fs::File;

        let file = File::open(wav_path)?;
        let mut reader = hound::WavReader::new(file)?;

        // Überprüfe Sample-Rate und Format
        let spec = reader.spec();
        if spec.channels != 1 {
            anyhow::bail!("WAV muss mono sein, hat {} Kanäle", spec.channels);
        }
        if spec.sample_rate != SAMPLE_RATE {
            anyhow::bail!(
                "WAV muss {} Hz sein, hat {} Hz",
                SAMPLE_RATE,
                spec.sample_rate
            );
        }

        let samples: Result<Vec<f32>, _> = reader
            .samples::<f32>()
            .map(|s| s.map_err(anyhow::Error::from))
            .collect();
        let samples = samples?;

        tracing::info!(
            wav_path = ?wav_path,
            samples = samples.len(),
            duration_s = format_args!("{:.2}", samples.len() as f64 / SAMPLE_RATE as f64),
            "WAV geladen, speise nun in Echtzeit-Geschwindigkeit ein"
        );

        // Speise Samples in Echtzeit ein: 16000 Samples pro Sekunde ≈ ~0,3 ms pro Sample
        let chunk_ms = 100; // Kleine Chunks, um Timing realistisch zu halten
        let chunk_samples =
            ((SAMPLE_RATE as u64 * chunk_ms) / 1000).max(1) as usize;
        let delay_between_chunks = Duration::from_millis(chunk_ms);

        for chunk in samples.chunks(chunk_samples) {
            self.append_to_ring(chunk);
            std::thread::sleep(delay_between_chunks);
        }

        tracing::info!("WAV vollständig eingefüttert");
        Ok(())
    }

    fn save_ring_to_wav(&self) -> Result<()> {
        if !self.save_audio {
            return Ok(());
        }

        // Verwende komplette audio_history statt nur des Ringbuffer-Tails.
        // So wird der gesamte aufgenommene Audio gespeichert, nicht nur die letzten
        // 20s aus dem Ring.
        if self.audio_history.is_empty() {
            tracing::debug!("Audio-History ist leer, Audio wird nicht gespeichert");
            return Ok(());
        }

        let output_dir = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
            .join("Library")
            .join("Logs")
            .join("Medivox");
        std::fs::create_dir_all(&output_dir)?;

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        let filename = output_dir.join(format!("medivox-recording-{}.wav", timestamp));

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };

        let mut writer = hound::WavWriter::create(&filename, spec)?;
        for &sample in &self.audio_history {
            writer.write_sample(sample)?;
        }
        writer.finalize()?;

        let duration_s = self.audio_history.len() as f64 / SAMPLE_RATE as f64;
        tracing::info!(
            path = ?filename,
            duration_s = format_args!("{:.2}", duration_s),
            samples = self.audio_history.len(),
            "Audio gespeichert (kompletter Stream)"
        );

        Ok(())
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

/// Gibt zurück, wie viele Tokens am Anfang von `candidate` übersprungen werden
/// sollen, weil sie bereits durch `committed` abgedeckt sind.
///
/// Beim Sliding-Window beginnt jede neue Hypothesis **vor** der committed-Frontier
/// (das Fenster reicht weiter zurück als der commit-Stand). Die committed-Tokens
/// befinden sich daher irgendwo **im Inneren** von `candidate`, nicht am Anfang.
/// Wir suchen deshalb nach einem Suffix von `committed` an beliebiger Stelle in
/// `candidate` und liefern die Position unmittelbar danach zurück.
fn find_committed_frontier(committed: &[String], candidate: &[String]) -> usize {
    if committed.is_empty() || candidate.is_empty() {
        return 0;
    }

    // Ankerlänge: Verwende bis zu 12 Tokens vom Ende von committed als Anker.
    // Länger = sicherer gegen Falsch-Treffer, aber fehleranfälliger bei ASR-Fehlern.
    let max_anchor = committed.len().min(12);
    let min_anchor = 3.min(committed.len());

    // Phase 1: Exakter Match – bevorzugt für fehlerfreie Transkriptionen.
    for anchor_len in (min_anchor..=max_anchor).rev() {
        let tail = &committed[committed.len() - anchor_len..];
        let scan_end = candidate.len().saturating_sub(anchor_len);
        for pos in 0..=scan_end {
            if tail == &candidate[pos..pos + anchor_len] {
                return pos + anchor_len;
            }
        }
    }

    // Phase 2: Fuzzy Match – toleriert kleine ASR-Fehler im Überlappbereich.
    for anchor_len in (min_anchor.max(4)..=max_anchor).rev() {
        let tail = &committed[committed.len() - anchor_len..];
        let scan_end = candidate.len().saturating_sub(anchor_len);
        // Bei längeren Ankern: etwas lockerer (mehr Fehler möglich), aber immer
        // noch streng genug um Falsch-Treffer zu vermeiden.
        let threshold: f32 = if anchor_len >= 8 { 0.75 } else { 0.80 };

        for pos in 0..=scan_end {
            let window = &candidate[pos..pos + anchor_len];
            let matches = tail.iter().zip(window).filter(|(a, b)| a == b).count();
            if matches as f32 / anchor_len as f32 >= threshold {
                tracing::debug!(
                    anchor_len,
                    pos,
                    matches,
                    threshold,
                    "fuzzy_frontier_found"
                );
                return pos + anchor_len;
            }
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
