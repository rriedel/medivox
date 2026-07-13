# medivox-shim (Rust)

Rust-Neuimplementierung des Windows-Tray-Shims -- Schwesterprojekt zu `../shim-dotnet`
(C#) und `../shim` (Python, Original). Alle drei sind funktional austauschbar; sie
sprechen dieselbe Engine-Schnittstelle (`../engine`, `POST /transcribe`, rohe
float32-PCM-Samples @ 16 kHz mono -> Plaintext). Siehe Root-README fuer den Gesamtkontext.

Voraussetzung: Rust (stable, `x86_64-pc-windows-msvc`), Windows.

Hotkey **Strg+Alt+Leertaste** startet/stoppt die Aufnahme (Tray-Icon wird rot waehrend
der Aufnahme), die Aufnahme geht an die Engine und der erkannte Text wird ins fokussierte
Fenster eingetippt.

> **Nur ein Shim gleichzeitig.** `RegisterHotKey` ist systemweit exklusiv: laeuft der
> .NET- oder Python-Shim bereits mit demselben Hotkey, scheitert der Start hier mit einem
> Fehler im Log. Zum parallelen Betrieb den Hotkey per `MEDIVOX_HOTKEY` umbiegen
> (z. B. `MEDIVOX_HOTKEY=Control+Shift+Space`).

## Entwicklung

```powershell
cd shim-rust
cargo run
```

Logs landen unter `%LocalAppData%\Medivox\logs` (Praefix `shim-rs-`, taeglich rollierend,
14 Tage Aufbewahrung) -- derselbe Ordner wie beim .NET-Shim, aber eigener Dateiname.

Env-Vars: `MEDIVOX_ENGINE_HOST` / `MEDIVOX_ENGINE_PORT` (Standard `127.0.0.1` / `8123`),
`MEDIVOX_LOG_LEVEL` (Standard `info`), `MEDIVOX_HOTKEY` (Standard `Control+Alt+Space`).

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
