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
mod transcription_flow;
mod tray;

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::MenuEvent;

use client::EngineClient;
use config::SAMPLE_RATE;
use recorder::Recorder;
use transcription_flow::{FlowConfig, TranscriptionFlow};
use tray::Tray;

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
    let pseudo_streaming_enabled = config::pseudo_streaming_enabled();
    let chunk_interval = Duration::from_millis(config::stream_chunk_ms());
    let min_audio_ms = config::stream_min_audio_ms();
    let min_chunk_samples = ((SAMPLE_RATE as u64 * min_audio_ms) / 1_000) as usize;
    let overlap_ms = config::stream_overlap_ms();
    let overlap_samples = ((SAMPLE_RATE as u64 * overlap_ms) / 1_000) as usize;
    let mut flow = TranscriptionFlow::new(FlowConfig {
        pseudo_streaming_enabled,
        chunk_interval,
        min_chunk_samples,
        min_audio_ms,
        overlap_ms,
        overlap_samples,
    });

    tracing::info!("Hotkey installed ({spec}), ready for activation");
    tracing::info!("to stop, use tray icon!");
    if flow.config().pseudo_streaming_enabled {
        tracing::info!(
            "Pseudo-Streaming aktiv: Tick {} ms, Min-Audio {} ms, Overlap {} ms",
            flow.config().chunk_interval.as_millis(),
            flow.config().min_audio_ms,
            flow.config().overlap_ms
        );
    } else {
        tracing::info!("Pseudo-Streaming deaktiviert: ganze Aufnahme wird am Ende transkribiert");
    }

    let hotkey_rx = GlobalHotKeyEvent::receiver();
    let menu_rx = MenuEvent::receiver();
    let quit_id = tray.quit_id.clone();

    event_loop.run(move |_event, _target, control_flow| {
        *control_flow = if let Some(next) = flow.next_wakeup(recording) {
            ControlFlow::WaitUntil(next)
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
                &mut flow,
            );
        }

        if recording {
            flow.process_tick(&mut recorder, &engine);
        }

        while let Ok(event) = menu_rx.try_recv() {
            if event.id == quit_id {
                if recording {
                    let _ = on_hotkey(
                        true,
                        &tray,
                        &mut recorder,
                        &engine,
                        &mut flow,
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
    flow: &mut TranscriptionFlow,
) -> bool {
    if !currently_recording {
        match recorder.start() {
            Ok(()) => {
                tray.set_recording(true);
                flow.on_recording_started();
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
            return false;
        }
    };
    flow.spawn_stop_transcription(tail_audio, Arc::clone(engine), inject::type_text);

    false
}
