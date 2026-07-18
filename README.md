# medivox

Lokale Spracherkennung fuer medizinisches Diktat unter Windows. Besteht aus zwei
Teilen:

- **engine** -- FastAPI-Server, der Audio per [faster-whisper](https://github.com/SYSTRAN/faster-whisper)
  transkribiert.
- **shim** -- Tray-/Menueleisten-Anwendung, die per globalem Hotkey Audio aufnimmt, an die
  Engine schickt und den erkannten Text ins fokussierte Fenster eintippt.

Voraussetzung: Windows *oder* macOS (Apple Silicon) fuer den Shim, Python 3.12 fuer die
Engine. Die Windows-Shims nutzen `user32`/`SendInput`, der macOS-Shim `CGEventPost` --
die plattformspezifischen Teile sind je Variante getrennt.

### Shim-Varianten

Den Shim gibt es viermal, funktional gleichwertig und gegeneinander austauschbar -- alle
sprechen dieselbe Engine-Schnittstelle. Es kann immer nur **einer gleichzeitig** pro
Rechner laufen: der globale Hotkey ist systemweit exklusiv.

| Verzeichnis | Plattform | Sprache | Groesse | RAM (Leerlauf) |
|---|---|---|---|---|
| [`shim/`](shim/) | Windows | Python 3.12 (Original) | venv | -- |
| [`shim-dotnet/`](shim-dotnet/) | Windows | C# / .NET 8 ([README](shim-dotnet/README.md)) | 68,5 MB (self-contained) | ~117 MB |
| [`shim-rust/`](shim-rust/) | Windows | Rust ([README](shim-rust/README.md)) | 2,0 MB (nativ) | ~15 MB |
| [`shim-macos/`](shim-macos/) | macOS (Apple Silicon) | Rust ([README](shim-macos/README.md)) | nativ | -- |

## engine

FastAPI-Server, hoert standardmaessig auf `127.0.0.1:8123` (siehe
[`medivox_engine/config.py`](engine/medivox_engine/config.py)).

Alle Befehle werden aus dem Verzeichnis `engine/` ausgefuehrt.

```powershell
cd engine

# venv anlegen
python -m venv .venv

# Abhaengigkeiten installieren
.venv\Scripts\python.exe -m pip install -r requirements.txt
# fuer Entwicklung (inkl. mypy)
.venv\Scripts\python.exe -m pip install -r requirements-dev.txt

# Server starten
.venv\Scripts\python.exe -m medivox_engine.main

# Typpruefung
.venv\Scripts\python.exe -m mypy --config-file mypy.ini
```

Endpunkte:

- `POST /transcribe` -- Body: rohe `float32`-PCM-Samples (16 kHz, mono). Antwort: erkannter Text als Plaintext.
- `POST /reload-glossary` -- laedt `glossary.txt` neu ein, ohne den Server neu zu starten.

### Performance-Tuning (CPU)

Die wichtigsten Stellschrauben fuer schnellere Transkription sind in
[`engine/medivox_engine/config.py`](engine/medivox_engine/config.py):

- `model_size` -- groesster Hebel fuer Geschwindigkeit (`medium` -> `small`/`base`)
- `cpu_threads` -- Anzahl CPU-Threads fuer CTranslate2
- `beam_size`, `best_of`, `temperature` -- Decoder-Geschwindigkeit/Qualitaet
- `without_timestamps`, `condition_on_previous_text` -- kann Decoding beschleunigen
- `vad_filter`, `vad_min_silence_duration_ms` -- ueberspringt stille Abschnitte

Praxis-Profile:

- **Schnell**: `model_size="small"`, `beam_size=1`, `best_of=1`, `temperature=0.0`,
  `without_timestamps=True`, `condition_on_previous_text=False`, `vad_filter=True`
- **Balanciert**: `model_size="medium"`, `beam_size=3`, `best_of=1`,
  `without_timestamps=True`, `vad_filter=False`
- **Genauer**: `model_size="medium"`, `beam_size=5`, `best_of=5`, `vad_filter=False`

### Benchmark

Zum Vergleichen von Konfigurationen mit realen Audiodaten:

```powershell
cd engine
.venv\Scripts\python.exe benchmark_transcription.py <audio.wav> --repeats 5
```

```bash
cd engine
./.venv/bin/python benchmark_transcription.py <audio.wav> --repeats 5
```

Unterstuetzte Formate: `.wav` (mono, 16 kHz), `.npy`, `.f32`.

Fachbegriffe fuer bessere Erkennung stehen in [`glossary.txt`](engine/glossary.txt),
ein Begriff pro Zeile, Zeilen mit `#` werden ignoriert.

## shim

Tray-Anwendung, die sich standardmaessig mit der Engine unter `127.0.0.1:8123`
verbindet (siehe [`medivox_shim/config.py`](shim/medivox_shim/config.py)).

Alle Befehle werden aus dem Verzeichnis `shim/` ausgefuehrt.

```powershell
cd shim

# venv anlegen
python -m venv .venv

# Abhaengigkeiten installieren
.venv\Scripts\python.exe -m pip install -r requirements.txt
# fuer Entwicklung (inkl. mypy)
.venv\Scripts\python.exe -m pip install -r requirements-dev.txt

# Anwendung starten (Tray-Icon)
.venv\Scripts\python.exe -m medivox_shim.main

# Typpruefung
.venv\Scripts\python.exe -m mypy --config-file mypy.ini
```

Bedienung: Hotkey **Strg+Umschalt+Leertaste** startet/stoppt die Aufnahme (Tray-Icon
wird rot waehrend der Aufnahme). Nach dem Stoppen wird die Aufnahme an die Engine
geschickt und der erkannte Text ins aktuell fokussierte Fenster eingetippt. Ueber
das Tray-Menuepunkt "Beenden" wird die Anwendung geschlossen.

Hinweis: Die Texteingabe per `SendInput` funktioniert nicht, wenn das Zielfenster
mit hoeheren Rechten (z. B. als Administrator) laeuft als der Shim-Prozess --
Windows UIPI blockiert das.
