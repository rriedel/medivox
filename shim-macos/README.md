# medivox-shim-macos (Rust)

macOS-Portierung des Shims fuer **Apple Silicon** -- Schwesterprojekt zu `../shim-rust`
(Windows/Rust), `../shim-dotnet` (C#) und `../shim` (Python, Original). Alle sprechen
dieselbe Engine-Schnittstelle (`../engine`, `POST /transcribe`, rohe float32-PCM-Samples
@ 16 kHz mono -> Plaintext). Siehe Root-README fuer den Gesamtkontext.

Voraussetzung: macOS auf Apple Silicon (`aarch64-apple-darwin`), Rust (stable) samt
Xcode Command Line Tools.

Hotkey **Control+Option+Leertaste** startet/stoppt die Aufnahme (Menueleisten-Icon wird rot
waehrend der Aufnahme), die Aufnahme geht an die Engine und der erkannte Text wird ins
fokussierte Fenster eingetippt.

Der Shim nutzt dabei Pseudo-Streaming: waehrend der Aufnahme werden Audio-Chunks bereits
laufend an die Engine geschickt. Beim Stoppen muss daher meist nur noch der Rest transkribiert
werden, was die gefuehlte Wartezeit deutlich verkuerzt.

Um Artefakte an Chunk-Grenzen zu reduzieren, wird zwischen aufeinanderfolgenden Chunks ein
konfigurierbarer Audio-Overlap verwendet.

Zum Qualitaetsvergleich kann Pseudo-Streaming komplett deaktiviert werden; dann wird wieder die
gesamte Aufnahme erst beim Stop transkribiert.

> **Nur ein Shim gleichzeitig pro Rechner.** Der globale Hotkey ist systemweit exklusiv.
> Zum Umbiegen `MEDIVOX_HOTKEY` setzen (z. B. `MEDIVOX_HOTKEY=Control+Shift+Space`).

## Berechtigungen (wichtig)

macOS verlangt zwei Freigaben, sonst funktioniert der Shim nur halb:

1. **Bedienungshilfen** (*Systemeinstellungen > Datenschutz & Sicherheit > Bedienungshilfen*):
   noetig fuer die Texteingabe (`CGEventPost`). Fehlt sie, verwirft macOS die
   Tastatur-Events **stillschweigend** -- die Aufnahme laeuft, aber es erscheint kein Text.
   Der Shim schreibt beim Start eine Warnung ins Log, wenn die Berechtigung fehlt.
   Als **App-Bundle** (siehe unten) wird **Medivox** selbst freigeschaltet; beim Start des
   nackten Binaries aus dem Terminal muss stattdessen das **Terminal** (bzw. die IDE)
   freigeschaltet werden, da das Binary als dessen Kindprozess laeuft.
2. **Mikrofon** (*Datenschutz & Sicherheit > Mikrofon*): wird beim ersten Aufnehmen
   automatisch abgefragt.

Der globale Hotkey selbst braucht **keine** Bedienungshilfen-Berechtigung -- er nutzt
Carbon `RegisterEventHotKey`.

## Voraussetzungen installieren

Einmalig pro Rechner:

```bash
# Xcode Command Line Tools (Linker + Frameworks)
xcode-select --install

# Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Alternativ Rust ueber Homebrew: `brew install rustup && rustup default stable`.

## Entwicklung

```bash
cd shim-macos
cargo run
```

Logs landen unter `~/Library/Logs/Medivox` (Praefix `shim-mac-`, taeglich rollierend,
14 Tage Aufbewahrung).

Env-Vars: `MEDIVOX_ENGINE_HOST` / `MEDIVOX_ENGINE_PORT` (Standard `127.0.0.1` / `8123`),
`MEDIVOX_LOG_LEVEL` (Standard `info`), `MEDIVOX_HOTKEY` (Standard `Control+Alt+Space`;
`MEDIVOX_PSEUDO_STREAMING` (Standard `true`, Werte z. B. `true/false`, `on/off`),
`Alt` = Option-Taste), `MEDIVOX_STREAM_CHUNK_MS` (Standard `800`, Bereich `200..5000`),
`MEDIVOX_STREAM_OVERLAP_MS` (Standard `250`, Bereich `0..2000`),
`MEDIVOX_STREAM_MIN_AUDIO_MS` (Standard `2400`, Bereich `400..10000`).

Der Shim loggt fuer jeden Engine-Request Metriken mit `transcribe_metrics`:
`kind` (`chunk`, `final_tail`, `full`), `audio_s`, `elapsed_s`, `rtf`, `chars`.

## Release-Build

```bash
cargo build --release
```

Ergebnis: `target/release/medivox-shim` -- ein natives Einzel-Binary. Fuer den Alltag
empfiehlt sich stattdessen das `.app`-Bundle (siehe unten): Menueleisten-Icon ohne
Terminal, eigener Eintrag in den Bedienungshilfen, korrekter Name im Mikrofon-Dialog.

## App-Bundle (empfohlen)

```bash
./make-app.sh
```

Das Skript baut den Release-Build, verpackt ihn zu `target/Medivox.app` und versieht das
Bundle mit einer **Ad-hoc-Signatur**. Ergebnis ist ein Menueleisten-Agent (`LSUIElement`,
kein Dock-Icon).

Einrichtung nach dem ersten Bauen:

```bash
open target/Medivox.app
```

1. Beim ersten Aufnehmen fragt macOS nach der **Mikrofon**-Berechtigung -> erlauben.
2. Die **Bedienungshilfen**-Berechtigung manuell erteilen: *Systemeinstellungen >
   Datenschutz & Sicherheit > Bedienungshilfen* -> **Medivox** aktivieren. Danach die App
   einmal beenden (Menueleiste > Beenden) und neu starten.

Die Ad-hoc-Signatur gibt dem Bundle eine stabile Code-Identitaet -- so bleibt die einmal
erteilte Bedienungshilfen-Freigabe auch nach einem `./make-app.sh`-Rebuild erhalten
(solange der Bundle-Identifier `com.medivox.shim` gleich bleibt). Zum automatischen Start
bei der Anmeldung die App unter *Systemeinstellungen > Allgemein > Anmeldeobjekte*
hinzufuegen.

Bundle-Konfiguration: [`bundle/Info.plist`](bundle/Info.plist).

## Modul-Uebersicht

Die Module entsprechen denen von `../shim-rust`; plattformportable Teile sind
unveraendert uebernommen.

| Datei | Zweck | Herkunft |
|---|---|---|
| `main.rs` | Entry Point, tao-Eventloop, Aufnahme-/Transkriptions-Ablauf | macOS-spezifisch (Eventloop statt Win32-Message-Loop) |
| `recorder.rs` | CoreAudio-Aufnahme (`cpal`), Downmix + Resampling auf 16 kHz (`rubato`) | unveraendert von `shim-rust` |
| `inject.rs` | `CGEventPost` fuer Unicode-Texteingabe (`core-graphics`) | macOS-spezifisch (Pendant zu SendInput) |
| `tray.rs` | Menueleisten-Icon + Menue (`tray-icon`), Icons zur Laufzeit gezeichnet | unveraendert von `shim-rust` |
| `client.rs` | HTTP-Client zur Engine (`ureq`, blockierend) | unveraendert von `shim-rust` |
| `config.rs` | Konfiguration/Defaults | unveraendert von `shim-rust` |
| `logging.rs` | `tracing` + rollierende Logdatei nach `~/Library/Logs/Medivox` | macOS-Pfade |

Der globale Hotkey braucht keine eigene Datei: `global-hotkey` kapselt Carbon
`RegisterEventHotKey`/`UnregisterEventHotKey` inkl. Aufraeumen beim Drop.

## Unterschiede zur Windows-Rust-Fassung (`../shim-rust`)

- **Eventloop statt Message-Loop**: `tao` pumpt die NSApplication-Runloop, in die sich
  `tray-icon` (NSStatusItem) und `global-hotkey` (Carbon) einhaengen. Statt der rohen
  `GetMessageW`-Schleife leeren wir die Event-Kanaele nach jedem Loop-Durchlauf.
- **Texteingabe ueber `CGEventPost`** statt `SendInput`: jeder Codepunkt wird als
  Key-Down/Key-Up-Paar mit Unicode-Nutzlast (`set_string`) gepostet -- layoutunabhaengig.
  Erfordert die Bedienungshilfen-Berechtigung.
- **Logs unter `~/Library/Logs/Medivox`** statt `%LocalAppData%\Medivox\logs`.
