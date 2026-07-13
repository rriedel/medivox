# medivox

Lokale Spracherkennung fuer medizinisches Diktat unter Windows. Besteht aus zwei
Teilen:

- **engine** -- FastAPI-Server, der Audio per [faster-whisper](https://github.com/SYSTRAN/faster-whisper)
  transkribiert.
- **shim** -- Windows-Tray-Anwendung, die per globalem Hotkey Audio aufnimmt, an die
  Engine schickt und den erkannten Text ins fokussierte Fenster eintippt.

Voraussetzung: Windows (jeder Shim nutzt `user32`/`SendInput` und ist nicht
plattformunabhaengig), Python 3.12 fuer die Engine.

### Shim-Varianten

Den Shim gibt es dreimal, funktional gleichwertig und gegeneinander austauschbar -- alle
sprechen dieselbe Engine-Schnittstelle. Es kann immer nur **einer gleichzeitig** laufen:
`RegisterHotKey` ist systemweit exklusiv.

| Verzeichnis | Sprache | Groesse | RAM (Leerlauf) |
|---|---|---|---|
| [`shim/`](shim/) | Python 3.12 (Original) | venv | -- |
| [`shim-dotnet/`](shim-dotnet/) | C# / .NET 8 ([README](shim-dotnet/README.md)) | 68,5 MB (self-contained) | ~117 MB |
| [`shim-rust/`](shim-rust/) | Rust ([README](shim-rust/README.md)) | 2,0 MB (nativ) | ~15 MB |

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
