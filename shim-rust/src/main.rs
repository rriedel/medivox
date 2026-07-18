// Kein Konsolenfenster beim Start (.NET-Pendant: <OutputType>WinExe</OutputType>).
#![windows_subsystem = "windows"]

//! Entry Point und Message-Loop (.NET-Pendant: Program.cs + der Ablaufteil von
//! TrayApplicationContext.cs).
//!
//! Statt der WinForms-Message-Loop (Application.Run) laeuft hier eine rohe
//! GetMessageW-Schleife. tray-icon und global-hotkey erzeugen intern jeweils ein
//! verstecktes Fenster auf dem aufrufenden Thread und brauchen genau das: einen Thread,
//! der Messages pumpt. Ihre Events landen in Kanaelen, die wir nach jedem
//! DispatchMessageW leeren -- so bleibt alles auf dem Main-Thread, ohne Send/Sync-Turnerei.

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
use tray_icon::menu::MenuEvent;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, PostQuitMessage, TranslateMessage, MSG,
};

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
    let tray = Tray::new()?;

    let manager = GlobalHotKeyManager::new().context("Hotkey-Manager konnte nicht starten")?;
    let spec = config::hotkey();
    let hotkey = HotKey::from_str(&spec)
        .with_context(|| format!("Hotkey '{spec}' konnte nicht geparst werden"))?;
    manager.register(hotkey).with_context(|| {
        format!("Hotkey '{spec}' konnte nicht registriert werden (evtl. bereits von einer anderen Anwendung belegt)")
    })?;

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
            "Pseudo-Streaming konfiguriert: Tick {} ms, Min-Audio {} ms, Overlap {} ms",
            flow.config().chunk_interval.as_millis(),
            flow.config().min_audio_ms,
            flow.config().overlap_ms
        );
    }

    let hotkey_rx = GlobalHotKeyEvent::receiver();
    let menu_rx = MenuEvent::receiver();

    let mut msg = MSG::default();
    // GetMessageW liefert 0 bei WM_QUIT und -1 bei Fehler.
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        while let Ok(event) = hotkey_rx.try_recv() {
            // Ohne diesen Filter wuerde jeder Tastendruck zweimal umschalten (Pressed +
            // Released) und die Aufnahme sofort wieder beenden.
            if event.state != HotKeyState::Pressed || event.id != hotkey.id() {
                continue;
            }
            recording = !recording;
            on_hotkey(recording, &tray, &mut recorder, &engine, &mut flow);
        }

        if recording {
            flow.process_tick(&mut recorder, &engine);
        }

        while let Ok(event) = menu_rx.try_recv() {
            if event.id == tray.quit_id {
                unsafe { PostQuitMessage(0) };
            }
        }
    }

    if recording {
        let _ = recorder.stop();
    }
    tracing::info!("Beendet");
    Ok(())
}

fn on_hotkey(
    recording: bool,
    tray: &Tray,
    recorder: &mut Recorder,
    engine: &Arc<EngineClient>,
    flow: &mut TranscriptionFlow,
) {
    if recording {
        match recorder.start() {
            Ok(()) => {
                tray.set_recording(true);
                flow.on_recording_started();
            }
            Err(err) => tracing::error!("Aufnahme konnte nicht starten: {err:#}"),
        }
        return;
    }

    tray.set_recording(false);
    let audio = match recorder.stop() {
        Ok(audio) => audio,
        Err(err) => {
            tracing::error!("Aufnahme konnte nicht beendet werden: {err:#}");
            return;
        }
    };
    if audio.is_empty() {
        return;
    }

    // Transkription + Texteingabe laufen weiterhin abseits der Message-Loop.
    flow.spawn_stop_transcription(audio, Arc::clone(engine), inject::type_text);
}
