# Text automatisch in LibreOffice-Writer-Vorlage einfuegen

Dieses kleine Setup fuegt den Inhalt einer reinen ASCII/Text-Datei per
Kommandozeile in eine LibreOffice-Writer-Vorlage ein und speichert das
Ergebnis als neues Dokument (.odt, .docx oder .pdf) - ganz ohne dass sich
ein LibreOffice-Fenster oeffnet.

## Enthaltene Dateien

- `fill_template.py` - das eigentliche Automatisierungs-Skript
- `vorlage.odt` - eine Beispiel-Vorlage mit dem Platzhalter `{{TEXT}}`
- `beispiel.txt` - ein Beispiel-Textinhalt zum Ausprobieren

## Einmalige Einrichtung (Windows)

Du brauchst keine zusaetzliche Software zu installieren - LibreOffice bringt
sein eigenes Python samt der noetigen "uno"-Automatisierungs-Bibliothek
bereits mit. Du musst nur **dieses** Python benutzen, nicht ein normal
installiertes Python:

```
C:\Program Files\LibreOffice\program\python.exe
```

Falls LibreOffice woanders installiert ist, findest du `python.exe` im
`program`-Unterordner deiner LibreOffice-Installation.

Tipp: Leg dir dafuer einen kurzen Alias/eine `.bat`-Datei an, z.B.
`lo_python.bat`:

```bat
@echo off
"C:\Program Files\LibreOffice\program\python.exe" %*
```

Dann kannst du im Rest dieser Anleitung einfach `lo_python fill_template.py ...`
statt des ganzen Pfades schreiben.

## Verwendung

```
"C:\Program Files\LibreOffice\program\python.exe" fill_template.py vorlage.odt beispiel.txt ausgabe.odt
```

Das erzeugt `ausgabe.odt` - eine Kopie der Vorlage, bei der `{{TEXT}}` durch
den Inhalt von `beispiel.txt` ersetzt wurde. Die Vorlage selbst bleibt
unveraendert, du kannst sie beliebig oft wiederverwenden.

Beim allerersten Aufruf startet das Skript im Hintergrund eine
LibreOffice-Instanz (dauert ein paar Sekunden). Diese bleibt danach aktiv,
sodass weitere Aufrufe deutlich schneller sind - ideal, wenn du das Skript
in einer Schleife fuer mehrere Dateien aufrufst.

### Andere Ausgabeformate

```
...fill_template.py vorlage.odt beispiel.txt ausgabe.docx
...fill_template.py vorlage.odt beispiel.txt ausgabe.pdf
```

Das Format wird automatisch aus der Dateiendung von `ausgabe.*` erkannt,
kann aber auch explizit mit `--format odt|docx|pdf` angegeben werden.

### Eigenen Platzhalter verwenden

Standardmaessig wird nach `{{TEXT}}` gesucht. Mit `--placeholder` kannst du
einen anderen Platzhalter angeben:

```
...fill_template.py vorlage.odt beispiel.txt ausgabe.odt --placeholder "[[INHALT]]"
```

## Eigene Vorlage erstellen

1. Oeffne LibreOffice Writer, gestalte das Dokument wie gewuenscht
   (Briefkopf, Logo, Formatierung, Kopf-/Fusszeilen ...).
2. Tippe an der Stelle, an der spaeter der Text eingefuegt werden soll,
   einfach `{{TEXT}}` (oder deinen eigenen Platzhalter).
3. Speichere als `.odt` (oder `.ott`, wenn du sie als "richtige" Vorlage
   fuehren willst - funktioniert genauso).

## Mehrere Dateien auf einmal (Batch)

Falls du spaeter doch mal mehrere Textdateien in einem Rutsch verarbeiten
willst, reicht eine einfache Schleife, z.B. in PowerShell:

```powershell
Get-ChildItem "C:\Pfad\zu\texten\*.txt" | ForEach-Object {
    $out = "C:\Pfad\zu\ausgabe\$($_.BaseName).odt"
    & "C:\Program Files\LibreOffice\program\python.exe" fill_template.py `
        vorlage.odt $_.FullName $out
}
```

Da die LibreOffice-Instanz nach dem ersten Aufruf im Hintergrund weiterlaeuft,
ist jeder weitere Durchlauf schnell.

## Hinweise

- Der Text in der `.txt`-Datei kann mehrzeilig sein - jede Zeile wird zu
  einem eigenen Absatz im Ergebnisdokument.
- Es koennen mehrere Vorkommen desselben Platzhalters in einer Vorlage
  stehen - alle werden ersetzt.
- Wird der Platzhalter nicht gefunden, bricht das Skript mit einer
  Fehlermeldung ab (Tippfehler im Platzhalter pruefen).
- Wenn du irgendwann die im Hintergrund laufende LibreOffice-Instanz
  beenden willst: im Taskmanager den Prozess `soffice.bin` beenden, oder
  einfach den PC neu starten. Sie kostet im Leerlauf kaum Ressourcen.
