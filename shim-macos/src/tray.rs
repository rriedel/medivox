//! Menueleisten-Icon und Kontextmenue (unveraendert uebernommen von
//! ../shim-rust/src/tray.rs).
//!
//! Die beiden Icons (grau = bereit, rot = Aufnahme) werden zur Laufzeit als RGBA-Puffer
//! gezeichnet -- so braucht das Projekt keine Icon-Dateien. tray-icon rendert den Puffer
//! auf macOS in ein NSStatusItem der Menueleiste.

use anyhow::{Context, Result};
use tray_icon::menu::{Menu, MenuId, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

const SIZE: u32 = 64;

pub struct Tray {
    icon: TrayIcon,
    gray: Icon,
    red: Icon,
    pub quit_id: MenuId,
}

impl Tray {
    pub fn new() -> Result<Self> {
        let gray = circle_icon(128, 128, 128)?;
        let red = circle_icon(220, 40, 40)?;

        let menu = Menu::new();
        let quit = MenuItem::new("Beenden", true, None);
        menu.append(&quit).context("Menueeintrag fehlgeschlagen")?;

        let icon = TrayIconBuilder::new()
            .with_icon(gray.clone())
            .with_tooltip("medivox -- bereit")
            .with_menu(Box::new(menu))
            .build()
            .context("Menueleisten-Icon konnte nicht erstellt werden")?;

        Ok(Self {
            icon,
            gray,
            red,
            quit_id: quit.id().clone(),
        })
    }

    pub fn set_recording(&self, active: bool) {
        let (icon, tip) = if active {
            (self.red.clone(), "medivox -- Aufnahme laeuft")
        } else {
            (self.gray.clone(), "medivox -- bereit")
        };
        if let Err(err) = self.icon.set_icon(Some(icon)) {
            tracing::warn!("Menueleisten-Icon konnte nicht gewechselt werden: {err}");
        }
        if let Err(err) = self.icon.set_tooltip(Some(tip)) {
            tracing::warn!("Menueleisten-Tooltip konnte nicht gesetzt werden: {err}");
        }
    }
}

/// Gefuellter Kreis mit weichem Rand (4x-Supersampling).
fn circle_icon(r: u8, g: u8, b: u8) -> Result<Icon> {
    let center = SIZE as f32 / 2.0;
    let radius = 24.0;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);

    for y in 0..SIZE {
        for x in 0..SIZE {
            let mut covered = 0u32;
            for sy in 0..4 {
                for sx in 0..4 {
                    let px = x as f32 + (sx as f32 + 0.5) / 4.0;
                    let py = y as f32 + (sy as f32 + 0.5) / 4.0;
                    let dx = px - center;
                    let dy = py - center;
                    if dx * dx + dy * dy <= radius * radius {
                        covered += 1;
                    }
                }
            }
            let alpha = (covered * 255 / 16) as u8;
            rgba.extend_from_slice(&[r, g, b, alpha]);
        }
    }

    Icon::from_rgba(rgba, SIZE, SIZE).context("Icon konnte nicht erzeugt werden")
}
