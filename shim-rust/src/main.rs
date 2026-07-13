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
mod tray;

use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Context, Result};
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use tray_icon::menu::MenuEvent;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, PostQuitMessage, TranslateMessage, MSG,
};

use client::EngineClient;
use recorder::Recorder;
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

    tracing::info!("Hotkey installed ({spec}), ready for activation");
    tracing::info!("to stop, use tray icon!");

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
            on_hotkey(recording, &tray, &mut recorder, &engine);
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

fn on_hotkey(recording: bool, tray: &Tray, recorder: &mut Recorder, engine: &Arc<EngineClient>) {
    if recording {
        match recorder.start() {
            Ok(()) => tray.set_recording(true),
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

    // Transkription + Texteingabe laufen abseits der Message-Loop, damit der Tray waehrend
    // der Engine-Anfrage nicht blockiert (.NET-Pendant: das fire-and-forget-Task in
    // HandleTranscriptionAsync). SendInput ist threadunabhaengig.
    let engine = Arc::clone(engine);
    std::thread::spawn(move || {
        let text = match engine.transcribe(&audio) {
            Ok(text) => text,
            Err(err) => {
                tracing::error!("Transkription fehlgeschlagen: {err:#}");
                return;
            }
        };

        tracing::info!("Transkriptionsergebnis: {text}");
        if text.is_empty() {
            return;
        }
        if let Err(err) = inject::type_text(&text) {
            tracing::error!("Texteingabe fehlgeschlagen: {err:#}");
        }
    });
}
