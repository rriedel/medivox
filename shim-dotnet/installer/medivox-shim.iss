; Inno Setup Skript fuer den medivox-Shim.
; Baubar per: ISCC.exe installer\medivox-shim.iss
; Erwartet eine bereits publizierte self-contained Single-File-Exe unter
; ..\publish\medivox-shim.exe (siehe README.md, "dotnet publish"-Befehl).
;
; Pro-User-Installation ohne Admin-Rechte: der Shim laeuft im User-Kontext (Tray-Icon,
; SendInput, RegisterHotKey) und muss dort auch installiert/gestartet werden.

#define MyAppName "Medivox Shim"
#define MyAppExeName "medivox-shim.exe"
#define MyAppPublisher "Medivox"
#ifndef MyAppVersion
  #define MyAppVersion "0.1.0"
#endif

[Setup]
AppId={{6C7E6E2B-6D63-4C7C-9C6B-4C6B7C6B3B6B}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={localappdata}\Medivox
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir=output
OutputBaseFilename=medivox-shim-setup
Compression=lzma2
SolidCompression=yes
UninstallDisplayIcon={app}\{#MyAppExeName}
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "german"; MessagesFile: "compiler:Languages\German.isl"

[Tasks]
Name: "autostart"; Description: "Medivox Shim bei Anmeldung automatisch starten"; Flags: checkedonce

[Files]
Source: "..\publish\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{userstartmenu}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{userstartup}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: autostart

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "Medivox Shim jetzt starten"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: filesandordirs; Name: "{localappdata}\Medivox\logs"
