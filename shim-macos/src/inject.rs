//! Injiziert Text als Unicode-Tastatureingaben ins fokussierte Fenster
//! (macOS-Pendant zu ../shim-rust/src/inject.rs, dort SendInput).
//!
//! Statt Win32 SendInput werden hier synthetische Tastatur-Events ueber CGEventPost
//! erzeugt. Jeder Codepunkt wird als eigenes Key-Down/Key-Up-Paar gepostet, dessen
//! Unicode-Nutzlast per set_string gesetzt wird -- unabhaengig vom aktiven
//! Tastaturlayout und inklusive Zeichen ausserhalb der BMP (set_string konvertiert
//! intern nach UTF-16 samt Surrogatpaaren).
//!
//! Wichtig: CGEventPost wirkt nur, wenn der Prozess unter
//! Systemeinstellungen > Datenschutz & Sicherheit > Bedienungshilfen freigeschaltet ist.
//! Fehlt die Berechtigung, verwirft macOS die Events stillschweigend (kein Fehlercode).

use anyhow::{anyhow, Result};
use core_graphics::event::{CGEvent, CGEventTapLocation};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

/// Prueft, ob der Prozess die Bedienungshilfen-Berechtigung besitzt. Ohne sie bleibt
/// die Texteingabe wirkungslos.
pub fn accessibility_trusted() -> bool {
    // AXIsProcessTrusted liefert 1, wenn der Prozess als "Bedienungshilfe" vertraut ist.
    unsafe { AXIsProcessTrusted() }
}

pub fn type_text(text: &str) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }

    // HIDSystemState laesst die Events so aussehen, als kaemen sie von echter Hardware --
    // das erwarten die meisten Ziel-Apps.
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).map_err(|_| {
        anyhow!("CGEventSource konnte nicht erstellt werden (Bedienungshilfen-Berechtigung fehlt?)")
    })?;

    // Zeilenumbrueche als Wagenruecklauf senden -- macOS erzeugt bei der Return-Taste \r.
    for ch in text.replace('\n', "\r").chars() {
        let s = ch.to_string();

        let down = CGEvent::new_keyboard_event(source.clone(), 0, true)
            .map_err(|_| anyhow!("Key-Down-Event konnte nicht erstellt werden"))?;
        down.set_string(&s);
        down.post(CGEventTapLocation::HID);

        let up = CGEvent::new_keyboard_event(source.clone(), 0, false)
            .map_err(|_| anyhow!("Key-Up-Event konnte nicht erstellt werden"))?;
        up.set_string(&s);
        up.post(CGEventTapLocation::HID);
    }

    Ok(())
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
}
