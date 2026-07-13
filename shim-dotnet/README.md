# medivox-shim (.NET)

C#/.NET-Neuimplementierung des Windows-Tray-Shims (Ersatz fuer den Python-Shim unter
`../shim`, siehe Root-README fuer den Gesamtkontext). Voraussetzung: .NET 8 SDK,
Windows.

Funktional identisch zum Python-Original: Hotkey **Strg+Alt+Leertaste** startet/stoppt
die Audioaufnahme (Tray-Icon wird rot waehrend der Aufnahme), die Aufnahme wird an die
Engine (`../engine`, Standard `127.0.0.1:8123`) geschickt und der erkannte Text ins
fokussierte Fenster eingetippt.

## Entwicklung

```powershell
cd shim-dotnet
dotnet build
dotnet run --project src\Medivox.Shim
```

Logs landen unter `%LocalAppData%\Medivox\logs`. Engine-Host/-Port sind per Env-Var
ueberschreibbar: `MEDIVOX_ENGINE_HOST`, `MEDIVOX_ENGINE_PORT` (Standard `127.0.0.1`/`8123`),
Log-Level per `MEDIVOX_LOG_LEVEL` (Standard `Information`).

## Self-contained Single-File-Build

Erzeugt eine einzelne `.exe` ohne .NET-Laufzeitabhaengigkeit auf der Zielmaschine:

```powershell
dotnet publish src\Medivox.Shim\Medivox.Shim.csproj -c Release -r win-x64 `
  --self-contained true `
  -p:PublishSingleFile=true `
  -p:IncludeNativeLibrariesForSelfExtract=true `
  -p:EnableCompressionInSingleFile=true `
  -o publish
```

Ergebnis: `publish\medivox-shim.exe` (~70 MB, keine weiteren Abhaengigkeiten). Hinweis:
NativeAOT wird fuer WinForms-Apps derzeit nicht unterstuetzt (Tray-Icon/Hotkey-Fenster
nutzen `System.Windows.Forms`), daher self-contained statt AOT.

## Installer

Voraussetzung: [Inno Setup 6](https://jrsoftware.org/isinfo.php).

```powershell
dotnet publish src\Medivox.Shim\Medivox.Shim.csproj -c Release -r win-x64 `
  --self-contained true -p:PublishSingleFile=true `
  -p:IncludeNativeLibrariesForSelfExtract=true -o publish
& "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe" installer\medivox-shim.iss
```

Ergebnis: `installer\output\medivox-shim-setup.exe`. Installiert pro-User (ohne
Admin-Rechte) nach `%LocalAppData%\Medivox`, legt optional eine Autostart-Verknuepfung
(`shell:startup`) an und registriert einen Standard-Uninstall-Eintrag. Stille
Installation fuer automatisierten Rollout: `medivox-shim-setup.exe /VERYSILENT
/TASKS=autostart`.

## Modul-Uebersicht

| Datei | Zweck |
|---|---|
| `Program.cs` | Entry Point, startet die WinForms-Message-Loop |
| `TrayApplicationContext.cs` | Tray-Icon, verstecktes Hotkey-Fenster, Aufnahme-/Transkriptions-Ablauf |
| `GlobalHotkey.cs` | `RegisterHotKey`/`UnregisterHotKey` P/Invoke |
| `Recorder.cs` | WASAPI-Audioaufnahme (NAudio) inkl. Downmix/Resampling auf 16 kHz mono |
| `TextInjector.cs` | `SendInput` P/Invoke fuer Unicode-Texteingabe |
| `EngineClient.cs` | HTTP-Client zur Engine (`POST /transcribe`) |
| `ShimConfig.cs` | Konfiguration/Defaults |
| `Logging.cs` | Serilog-Setup (Console + rollierende Logdatei) |
