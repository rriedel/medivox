# medivox-shim (Rust)

Rust-Neuimplementierung des Windows-Tray-Shims -- Schwesterprojekt zu `../shim-dotnet`
(C#) und `../shim` (Python, Original). Alle drei sind funktional austauschbar; sie
sprechen dieselbe Engine-Schnittstelle (`../engine`, `POST /transcribe`, rohe
float32-PCM-Samples @ 16 kHz mono -> Plaintext). Siehe Root-README fuer den Gesamtkontext.

Voraussetzung: Windows, Rust (stable, `x86_64-pc-windows-msvc`) samt MSVC-Buildumgebung --
Installationsanleitung siehe [Voraussetzungen installieren](#voraussetzungen-installieren).

Hotkey **Strg+Alt+Leertaste** startet/stoppt die Aufnahme (Tray-Icon wird rot waehrend
der Aufnahme), die Aufnahme geht an die Engine und der erkannte Text wird ins fokussierte
Fenster eingetippt.

Wichtig fuer die Weiterentwicklung: Die Engine-Ansteuerung ist jetzt explizit in
`src/transcription_flow.rs` gekapselt. `main.rs` enthaelt nur noch den
plattformspezifischen Runtime-Teil (Win32-Message-Loop, Hotkey, Tray).

> **Nur ein Shim gleichzeitig.** `RegisterHotKey` ist systemweit exklusiv: laeuft der
> .NET- oder Python-Shim bereits mit demselben Hotkey, scheitert der Start hier mit einem
> Fehler im Log. Zum parallelen Betrieb den Hotkey per `MEDIVOX_HOTKEY` umbiegen
> (z. B. `MEDIVOX_HOTKEY=Control+Shift+Space`).

## Voraussetzungen installieren

Einmalig pro Rechner. Es werden zwei Dinge gebraucht:

1. **MSVC-Buildumgebung** -- das Standard-Rust-Target `x86_64-pc-windows-msvc` linkt gegen
   den MSVC-Linker (`link.exe`) und braucht das Windows SDK. Beides steckt in den Visual
   Studio Build Tools; eine vollstaendige Visual-Studio-Installation ist nicht noetig.
2. **Rust** via `rustup`.

Am einfachsten per [winget](https://learn.microsoft.com/windows/package-manager/) (in
Windows 11 vorinstalliert):

```powershell
# 1. Build Tools mit C++-Workload und Windows SDK (~3-4 GB, laeuft einige Minuten)
winget install --id Microsoft.VisualStudio.2022.BuildTools `
  --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"

# 2. Rust (rustup + stable-Toolchain). Erst NACH den Build Tools ausfuehren,
#    sonst kann rustup-init die MSVC-Umgebung nicht finden.
winget install --id Rustlang.Rustup
```

Danach eine **neue** PowerShell oeffnen (damit `%USERPROFILE%\.cargo\bin` im PATH ist) und
pruefen:

```powershell
rustup default stable   # falls noch keine aktive Toolchain gesetzt ist
rustc --version         # sollte z. B. "rustc 1.97.0 ..." zeigen
```

Alternativen ohne winget: Build Tools als [Standalone-Installer](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022)
(Workload "Desktopentwicklung mit C++"), Rust ueber [rustup.rs](https://rustup.rs/).

## Entwicklung

```powershell
cd shim-rust
cargo run
```

Logs landen unter `%LocalAppData%\Medivox\logs` (Praefix `shim-rs-`, taeglich rollierend,
14 Tage Aufbewahrung) -- derselbe Ordner wie beim .NET-Shim, aber eigener Dateiname.

Env-Vars: `MEDIVOX_ENGINE_HOST` / `MEDIVOX_ENGINE_PORT` (Standard `127.0.0.1` / `8123`),
`MEDIVOX_LOG_LEVEL` (Standard `info`), `MEDIVOX_HOTKEY` (Standard `Control+Alt+Space`),
`MEDIVOX_PSEUDO_STREAMING` (Standard `false`), `MEDIVOX_STREAM_CHUNK_MS`,
`MEDIVOX_STREAM_OVERLAP_MS`, `MEDIVOX_STREAM_MIN_AUDIO_MS`.

Hinweis: Im Windows-Rust-Shim bleibt das Verhalten aktuell bewusst bei Full-Utterance.
Pseudo-Streaming-Parameter sind als kompatible Schnittstelle bereits vorhanden, damit die
Streaming-Logik aus `shim-macos/src/transcription_flow.rs` spaeter 1:1 portiert werden kann.

## Release-Build

```powershell
cargo build --release
```

Ergebnis: `target\release\medivox-shim.exe` -- eine native Einzel-Exe ohne
Laufzeitabhaengigkeit auf der Zielmaschine.

## Modul-Uebersicht

Die Module entsprechen 1:1 denen des .NET-Shims, um den Vergleich einfach zu halten.

| Datei | Zweck | .NET-Pendant |
|---|---|---|
| `main.rs` | Entry Point, Win32-Message-Loop, Aufnahme-/Transkriptions-Ablauf | `Program.cs` + `TrayApplicationContext.cs` |
| `transcription_flow.rs` | Portierbare Engine-Ansteuerung + Metrik-Logging (aktuell Full-Utterance) | vorbereitet fuer Streaming-Port |
| `tray.rs` | Tray-Icon + Kontextmenue (`tray-icon`), Icons zur Laufzeit gezeichnet | `TrayApplicationContext.cs` |
| `recorder.rs` | WASAPI-Aufnahme (`cpal`), Downmix + Resampling auf 16 kHz (`rubato`) | `Recorder.cs` (NAudio) |
| `inject.rs` | `SendInput` fuer Unicode-Texteingabe (`windows`) | `TextInjector.cs` |
| `client.rs` | HTTP-Client zur Engine (`ureq`, blockierend) | `EngineClient.cs` |
| `config.rs` | Konfiguration/Defaults | `ShimConfig.cs` |
| `logging.rs` | `tracing` + rollierende Logdatei | `Logging.cs` (Serilog) |

Der globale Hotkey braucht hier keine eigene Datei: `global-hotkey` kapselt
`RegisterHotKey`/`UnregisterHotKey` inkl. Aufraeumen beim Drop (.NET-Pendant:
`GlobalHotkey.cs`).

## Vergleich mit der .NET-Fassung

Gemessen auf diesem Rechner, jeweils Release-/Publish-Build, im Leerlauf (Tray steht,
keine Aufnahme):

| | Rust | .NET 8 (self-contained) |
|---|---|---|
| Groesse der .exe | 2,0 MB | 68,5 MB |
| RAM Working Set | ~15 MB | ~117 MB |
| RAM privat | ~5,6 MB | ~49 MB |
| Laufzeit auf Zielmaschine | keine | keine (Runtime ist eingebettet) |

Der .NET-Shim kann nicht per NativeAOT gebaut werden (WinForms wird dort nicht
unterstuetzt), daher der Groessenunterschied.

## Unterschiede zur .NET-Fassung

- **Message-Loop von Hand**: statt `Application.Run` (WinForms) laeuft eine rohe
  `GetMessageW`-Schleife. `tray-icon` und `global-hotkey` erzeugen intern ein verstecktes
  Fenster auf dem aufrufenden Thread und brauchen nur einen Thread, der Messages pumpt.
- **Downmix im Callback**: der Aufnahmepuffer haelt bereits Mono-`f32` statt roher
  Geraetebytes -- rund halb so viel Speicher wie die `List<byte>` der .NET-Fassung.
- **`SendInput` ohne Fallstrick**: die `INPUT`-Union kommt aus den Windows-Headern, `cbSize`
  stimmt per Konstruktion. Der `ERROR_INVALID_PARAMETER` aus der C#-Fassung (unvollstaendige
  Union) kann hier nicht auftreten.
