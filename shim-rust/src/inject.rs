//! Injiziert Text als Unicode-Tastatureingaben ins fokussierte Fenster
//! (.NET-Pendant: TextInjector.cs).
//!
//! Funktioniert nicht, wenn das Zielfenster mit hoeheren Rechten laeuft als dieser
//! Prozess -- Windows UIPI blockiert das dann.
//!
//! Anders als beim P/Invoke-Nachbau in C# ist die INPUT-Union hier die echte Struktur
//! aus den Windows-Headern: cbSize stimmt per Konstruktion, der ERROR_INVALID_PARAMETER
//! aus der C#-Fassung kann hier nicht auftreten.

use std::mem::size_of;

use anyhow::{bail, Result};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
    VIRTUAL_KEY,
};

fn char_input(unit: u16, key_up: bool) -> INPUT {
    let mut flags = KEYEVENTF_UNICODE;
    if key_up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            // wVk muss 0 sein, wenn KEYEVENTF_UNICODE gesetzt ist; wScan traegt die
            // UTF-16-Codeeinheit.
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: unit,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

pub fn type_text(text: &str) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }

    // Ueber UTF-16-Codeeinheiten iterieren, nicht ueber chars: Zeichen ausserhalb der BMP
    // muessen als Surrogatpaar (zwei Events) gehen.
    let mut inputs: Vec<INPUT> = Vec::with_capacity(text.len() * 2);
    for unit in text.replace('\n', "\r").encode_utf16() {
        inputs.push(char_input(unit, false));
        inputs.push(char_input(unit, true));
    }

    let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    if sent as usize != inputs.len() {
        let error = unsafe { windows::Win32::Foundation::GetLastError() };
        bail!(
            "SendInput hat nur {} von {} Eingaben zugestellt. Win32-Fehler: {:?}",
            sent,
            inputs.len(),
            error
        );
    }
    Ok(())
}
