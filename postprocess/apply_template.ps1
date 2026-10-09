<#
.SYNOPSIS
    Fuegt eine Textdatei in die Gutachten-LibreOffice-Vorlage ein (siehe README.md).

.DESCRIPTION
    Wrapper um fill_template.py: ruft es mit dem LibreOffice-eigenen Python auf,
    damit man nicht jedes Mal den vollen Pfad und die Vorlage angeben muss.
    Die Ausgabedatei landet standardmaessig im selben Verzeichnis wie die
    Textdatei, mit gleichem Basisnamen und der Endung des Ausgabeformats.

.PARAMETER TextFile
    Pfad zur einzufuegenden .txt-Datei (ASCII/UTF-8).

.PARAMETER Template
    Pfad zur Vorlage. Default: Gutachten.ott im selben Verzeichnis wie dieses Skript.

.PARAMETER OutputFile
    Pfad der zu erzeugenden Ausgabedatei. Default: TextFile mit Endung des
    Ausgabeformats, im selben Verzeichnis wie die Textdatei.

.PARAMETER OutputFormat
    odt | docx | pdf. Default: odt.

.PARAMETER Placeholder
    Platzhalter-String in der Vorlage. Default: {{TEXT}}

.EXAMPLE
    .\apply_template.ps1 C:\Pfad\zu\text.txt

.EXAMPLE
    .\apply_template.ps1 -TextFile text.txt -OutputFormat docx
#>

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$TextFile,

    [string]$Template = (Join-Path $PSScriptRoot "Gutachten.ott"),

    [string]$OutputFile,

    [ValidateSet("odt", "docx", "pdf")]
    [string]$OutputFormat = "odt",

    [string]$Placeholder = "{{TEXT}}"
)

$ErrorActionPreference = "Stop"

function Find-LibreOfficePython {
    $candidates = @(
        "C:\Program Files\LibreOffice\program\python.exe",
        "C:\Program Files (x86)\LibreOffice\program\python.exe"
    )
    foreach ($c in $candidates) {
        if (Test-Path $c) {
            return $c
        }
    }
    throw "Kein LibreOffice-Python gefunden. Bitte LibreOffice installieren oder " +
          "den Pfad in apply_template.ps1 (Find-LibreOfficePython) anpassen."
}

if (-not (Test-Path $TextFile)) {
    throw "Textdatei nicht gefunden: $TextFile"
}
if (-not (Test-Path $Template)) {
    throw "Vorlage nicht gefunden: $Template"
}

if (-not $OutputFile) {
    $textFileItem = Get-Item $TextFile
    $OutputFile = Join-Path $textFileItem.DirectoryName "$($textFileItem.BaseName).$OutputFormat"
}

$loPython = Find-LibreOfficePython
$fillTemplateScript = Join-Path $PSScriptRoot "fill_template.py"

& $loPython $fillTemplateScript $Template $TextFile $OutputFile `
    --placeholder $Placeholder `
    --format $OutputFormat

if ($LASTEXITCODE -ne 0) {
    throw "fill_template.py ist mit Exit-Code $LASTEXITCODE fehlgeschlagen."
}
