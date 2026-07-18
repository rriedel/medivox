//! Entry Point und Eventloop (macOS-Pendant zu ../shim-rust/src/main.rs).
//!
//! Statt der rohen Win32-GetMessageW-Schleife laeuft hier eine tao-Eventloop, die die
//! NSApplication-Runloop pumpt. tray-icon (NSStatusItem) und global-hotkey (Carbon
//! RegisterEventHotKey) haengen ihre Events in diese Runloop ein; wir leeren ihre Kanaele
//! nach jedem Loop-Durchlauf. So bleibt alles auf dem Main-Thread.

mod client;
mod config;
mod inject;
mod logging;
mod recorder;
mod tray;

use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::MenuEvent;

use client::EngineClient;
use config::SAMPLE_RATE;
use recorder::Recorder;
use tray::Tray;

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

fn main() -> Result<()> {
    let _guard = logging::configure()?;

    if let Err(err) = run() {
        tracing::error!("Unerwarteter Absturz: {err:#}");
        return Err(err);
    }
    Ok(())
}

fn run() -> Result<()> {
    // Die Eventloop muss vor Tray und Hotkey-Manager existieren: beide haengen sich in
    // die davon initialisierte NSApplication-Runloop ein.
    let event_loop = EventLoopBuilder::new().build();

    let tray = Tray::new()?;

    let manager = GlobalHotKeyManager::new().context("Hotkey-Manager konnte nicht starten")?;
    let spec = config::hotkey();
    let hotkey = HotKey::from_str(&spec)
        .with_context(|| format!("Hotkey '{spec}' konnte nicht geparst werden"))?;
    manager.register(hotkey).with_context(|| {
        format!("Hotkey '{spec}' konnte nicht registriert werden (evtl. bereits von einer anderen Anwendung belegt)")
    })?;

    if !inject::accessibility_trusted() {
        tracing::warn!(
            "Bedienungshilfen-Berechtigung fehlt: Die Texteingabe bleibt wirkungslos, bis der \
             Prozess unter Systemeinstellungen > Datenschutz & Sicherheit > Bedienungshilfen \
             freigeschaltet ist."
        );
    }

    let engine = Arc::new(EngineClient::new());
    let mut recorder = Recorder::new();
    let mut recording = false;
    let mut session: Option<StreamingSession> = None;
    let pseudo_streaming_enabled = config::pseudo_streaming_enabled();
    let chunk_interval = Duration::from_millis(config::stream_chunk_ms());
    let min_audio_ms = config::stream_min_audio_ms();
    let min_chunk_samples = ((SAMPLE_RATE as u64 * min_audio_ms) / 1_000) as usize;
    let overlap_ms = config::stream_overlap_ms();
    let overlap_samples = ((SAMPLE_RATE as u64 * overlap_ms) / 1_000) as usize;

    tracing::info!("Hotkey installed ({spec}), ready for activation");
    tracing::info!("to stop, use tray icon!");
    if pseudo_streaming_enabled {
        tracing::info!(
            "Pseudo-Streaming aktiv: Tick {} ms, Min-Audio {} ms, Overlap {} ms",
            chunk_interval.as_millis(),
            min_audio_ms,
            overlap_ms
        );
    } else {
        tracing::info!("Pseudo-Streaming deaktiviert: ganze Aufnahme wird am Ende transkribiert");
    }

    let hotkey_rx = GlobalHotKeyEvent::receiver();
    let menu_rx = MenuEvent::receiver();
    let quit_id = tray.quit_id.clone();

    event_loop.run(move |_event, _target, control_flow| {
        *control_flow = if recording && pseudo_streaming_enabled {
            if let Some(active) = &session {
                ControlFlow::WaitUntil(active.next_tick)
            } else {
                ControlFlow::WaitUntil(Instant::now() + chunk_interval)
            }
        } else {
            ControlFlow::Wait
        };

        while let Ok(event) = hotkey_rx.try_recv() {
            // Ohne diesen Filter wuerde jeder Tastendruck zweimal umschalten (Pressed +
            // Released) und die Aufnahme sofort wieder beenden.
            if event.state != HotKeyState::Pressed || event.id != hotkey.id() {
                continue;
            }
            recording = on_hotkey(
                recording,
                &tray,
                &mut recorder,
                &engine,
                &mut session,
                pseudo_streaming_enabled,
                chunk_interval,
                min_chunk_samples,
                overlap_samples,
            );
        }

        if recording && pseudo_streaming_enabled {
            process_streaming_tick(&mut recorder, &engine, &mut session);
        }

        while let Ok(event) = menu_rx.try_recv() {
            if event.id == quit_id {
                if recording {
                    let _ = on_hotkey(
                        true,
                        &tray,
                        &mut recorder,
                        &engine,
                        &mut session,
                        pseudo_streaming_enabled,
                        chunk_interval,
                        min_chunk_samples,
                        overlap_samples,
                    );
                    recording = false;
                }
                tracing::info!("Beendet");
                *control_flow = ControlFlow::Exit;
            }
        }
    });
}

fn on_hotkey(
    currently_recording: bool,
    tray: &Tray,
    recorder: &mut Recorder,
    engine: &Arc<EngineClient>,
    session: &mut Option<StreamingSession>,
    pseudo_streaming_enabled: bool,
    chunk_interval: Duration,
    min_chunk_samples: usize,
    overlap_samples: usize,
) -> bool {
    if !currently_recording {
        match recorder.start() {
            Ok(()) => {
                tray.set_recording(true);
                if pseudo_streaming_enabled {
                    *session = Some(StreamingSession::new(
                        chunk_interval,
                        min_chunk_samples,
                        overlap_samples,
                    ));
                } else {
                    *session = None;
                }
                return true;
            }
            Err(err) => {
                tracing::error!("Aufnahme konnte nicht starten: {err:#}");
                return false;
            }
        }
    }

    tray.set_recording(false);
    let tail_audio = match recorder.stop() {
        Ok(audio) => audio,
        Err(err) => {
            tracing::error!("Aufnahme konnte nicht beendet werden: {err:#}");
            *session = None;
            return false;
        }
    };
    let active_session = session.take();
    let pseudo_active = pseudo_streaming_enabled;

    // Finalisierung laeuft abseits der Eventloop, damit die Menueleiste frei bleibt.
    let engine = Arc::clone(engine);
    std::thread::spawn(move || {
        if !pseudo_active {
            let start = Instant::now();
            let text = match engine.transcribe(&tail_audio) {
                Ok(text) => {
                    log_transcribe_metrics("full", None, tail_audio.len(), start.elapsed(), text.len());
                    text
                }
                Err(err) => {
                    log_transcribe_error_metrics("full", None, tail_audio.len(), start.elapsed(), &format!("{err:#}"));
                    tracing::error!("Transkription fehlgeschlagen: {err:#}");
                    return;
                }
            };

            let text = text.trim().to_string();
            tracing::info!("Transkriptionsergebnis: {text}");
            if text.is_empty() {
                return;
            }
            if let Err(err) = inject::type_text(&text) {
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
        if let Err(err) = inject::type_text(&text) {
            tracing::error!("Texteingabe fehlgeschlagen: {err:#}");
        }
    });

    false
}

fn process_streaming_tick(
    recorder: &mut Recorder,
    engine: &Arc<EngineClient>,
    session: &mut Option<StreamingSession>,
) {
    let Some(active) = session.as_mut() else {
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
