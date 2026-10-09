#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
fill_template.py
=================

Fuegt reinen ASCII/Text-Inhalt einer .txt-Datei in eine LibreOffice-Writer-
Vorlage ein (an einer Platzhalter-Stelle wie {{TEXT}}) und speichert das
Ergebnis als neues Dokument (odt/docx/pdf) - komplett per CLI steuerbar,
ohne dass LibreOffice sichtbar aufgehen muss.

Funktionsweise:
  1. Falls noch keine LibreOffice-Instanz im "Listen-Modus" laeuft, wird
     im Hintergrund eine gestartet (soffice --headless --accept=...).
     Das dauert beim allerersten Aufruf ein paar Sekunden.
  2. Ueber die UNO-Schnittstelle (das Automatisierungs-API von LibreOffice)
     verbindet sich das Skript mit dieser Instanz.
  3. Die Vorlage wird unsichtbar geladen, der Platzhalter wird durch den
     Inhalt der Textdatei ersetzt (Zeilenumbrueche werden zu echten
     Absaetzen), und das Ergebnis wird unter einem neuen Dateinamen
     gespeichert. Die Vorlage selbst wird NICHT veraendert.
  4. Die LibreOffice-Instanz bleibt im Hintergrund aktiv, damit weitere
     Aufrufe (z.B. in einer Schleife/Batch) schnell sind (kein erneuter
     Programmstart noetig).

Voraussetzung: Das Skript muss mit dem Python ausgefuehrt werden, das
LibreOffice mitbringt (das hat das "uno"-Modul bereits eingebaut):

  Windows:
    "C:\\Program Files\\LibreOffice\\program\\python.exe" fill_template.py ...

  Linux:
    /usr/bin/python3 fill_template.py ...   (falls python3-uno installiert ist)
    oder: /opt/libreoffice*/program/python fill_template.py ...

  macOS:
    /Applications/LibreOffice.app/Contents/Resources/python fill_template.py ...

Beispiel:
  python.exe fill_template.py vorlage.odt text.txt ausgabe.odt

Optionen:
  --placeholder "{{TEXT}}"   Platzhalter-String in der Vorlage (Default: {{TEXT}})
  --format odt|docx|pdf      Ausgabeformat (Default: aus Dateiendung von OUTPUT geraten)
  --port 2002                Port fuer die UNO-Verbindung (Default: 2002)
  --soffice PATH             Pfad zu soffice(.exe), falls nicht automatisch gefunden
  --no-autostart              Nicht automatisch eine LibreOffice-Instanz starten
                               (nuetzlich, wenn man sie selbst separat betreibt)
"""

import argparse
import os
import platform
import subprocess
import sys
import time

# ---------------------------------------------------------------------------
# UNO-Import erst hier, damit das Skript auch mit --help unter "normalem"
# Python (ohne uno-Modul) eine verstaendliche Fehlermeldung ausgibt.
# ---------------------------------------------------------------------------
try:
    import uno
    from com.sun.star.beans import PropertyValue
    from com.sun.star.text.ControlCharacter import PARAGRAPH_BREAK
except ImportError:
    uno = None


FILTER_MAP = {
    "odt": "writer8",
    "docx": "MS Word 2007 XML",
    "pdf": "writer_pdf_Export",
}


def make_prop(name, value):
    p = PropertyValue()
    p.Name = name
    p.Value = value
    return p


def find_soffice_binary(explicit_path=None):
    if explicit_path:
        return explicit_path

    system = platform.system()
    candidates = []
    if system == "Windows":
        candidates = [
            r"C:\Program Files\LibreOffice\program\soffice.exe",
            r"C:\Program Files (x86)\LibreOffice\program\soffice.exe",
        ]
    elif system == "Darwin":
        candidates = ["/Applications/LibreOffice.app/Contents/MacOS/soffice"]
    else:
        candidates = ["/usr/bin/soffice", "/usr/bin/libreoffice"]

    for c in candidates:
        if os.path.exists(c):
            return c

    # letzter Versuch: einfach "soffice" ueber PATH
    return "soffice"


def start_listener(soffice_path, port, user_profile_dir=None):
    """Startet LibreOffice headless im Hintergrund, lauschend auf UNO-Verbindungen."""
    args = [
        soffice_path,
        "--headless",
        "--invisible",
        "--nocrashreport",
        "--nodefault",
        "--norestore",
        "--nologo",
        "--nofirststartwizard",
        f"--accept=socket,host=localhost,port={port};urp;",
    ]
    if user_profile_dir:
        args.append(f"-env:UserInstallation=file://{user_profile_dir}")

    kwargs = {}
    if platform.system() == "Windows":
        # eigener Prozess, unabhaengig vom aufrufenden Skript, kein Konsolenfenster
        DETACHED_PROCESS = 0x00000008
        CREATE_NEW_PROCESS_GROUP = 0x00000200
        kwargs["creationflags"] = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
    else:
        kwargs["start_new_session"] = True

    subprocess.Popen(
        args,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        stdin=subprocess.DEVNULL,
        **kwargs,
    )


def connect(port, retries=30, delay=1.0):
    """Baut die UNO-Verbindung zu einer laufenden LibreOffice-Instanz auf."""
    local_ctx = uno.getComponentContext()
    resolver = local_ctx.ServiceManager.createInstanceWithContext(
        "com.sun.star.bridge.UnoUrlResolver", local_ctx
    )
    url = (
        f"uno:socket,host=localhost,port={port};"
        "urp;StarOffice.ComponentContext"
    )
    last_err = None
    for _ in range(retries):
        try:
            ctx = resolver.resolve(url)
            return ctx
        except Exception as e:  # noqa: BLE001 - UNO wirft eigene Exception-Typen
            last_err = e
            time.sleep(delay)
    raise RuntimeError(
        f"Konnte nach {retries * delay:.0f}s keine Verbindung zu LibreOffice "
        f"auf Port {port} aufbauen. Letzter Fehler: {last_err}"
    )


def get_desktop(ctx):
    smgr = ctx.ServiceManager
    return smgr.createInstanceWithContext("com.sun.star.frame.Desktop", ctx)


def path_to_url(path):
    return uno.systemPathToFileUrl(os.path.abspath(path))


def replace_placeholder_with_text(doc, placeholder, text_content):
    """
    Ersetzt jedes Vorkommen von `placeholder` im Dokument durch den Inhalt
    von `text_content`. Zeilenumbrueche im Text werden dabei zu echten
    Absatzwechseln (nicht nur zu einem einzelnen Absatz mit \\n drin).
    """
    search = doc.createSearchDescriptor()
    search.SearchString = placeholder
    found_ranges = doc.findAll(search)

    if found_ranges.getCount() == 0:
        raise ValueError(
            f"Platzhalter '{placeholder}' wurde in der Vorlage nicht gefunden."
        )

    lines = text_content.splitlines() or [""]

    for i in range(found_ranges.getCount()):
        text_range = found_ranges.getByIndex(i)
        text_obj = text_range.getText()
        cursor = text_obj.createTextCursorByRange(text_range)

        # Platzhalter-Text loeschen, Cursor bleibt an der Stelle stehen
        cursor.setString("")

        for idx, line in enumerate(lines):
            if idx > 0:
                text_obj.insertControlCharacter(cursor, PARAGRAPH_BREAK, False)
            text_obj.insertString(cursor, line, False)


def save_document(doc, output_path, fmt):
    filter_name = FILTER_MAP.get(fmt)
    if not filter_name:
        raise ValueError(
            f"Unbekanntes Ausgabeformat '{fmt}'. "
            f"Erlaubt: {', '.join(FILTER_MAP)}"
        )
    props = (make_prop("FilterName", filter_name), make_prop("Overwrite", True))
    doc.storeToURL(path_to_url(output_path), props)


def guess_format(output_path, explicit_fmt):
    if explicit_fmt:
        return explicit_fmt
    ext = os.path.splitext(output_path)[1].lstrip(".").lower()
    if ext in FILTER_MAP:
        return ext
    raise ValueError(
        "Konnte das Ausgabeformat nicht aus der Dateiendung ableiten - "
        "bitte --format odt|docx|pdf angeben."
    )


def main():
    parser = argparse.ArgumentParser(
        description="Fuegt eine Textdatei in eine LibreOffice-Writer-Vorlage ein."
    )
    parser.add_argument("template", help="Pfad zur Vorlage (.odt/.ott/.docx ...)")
    parser.add_argument("textfile", help="Pfad zur einzufuegenden .txt-Datei (ASCII/UTF-8)")
    parser.add_argument("output", help="Pfad der zu erzeugenden Ausgabedatei")
    parser.add_argument("--placeholder", default="{{TEXT}}",
                         help="Platzhalter-String in der Vorlage (Default: {{TEXT}})")
    parser.add_argument("--format", choices=list(FILTER_MAP), default=None,
                         help="Ausgabeformat, sonst aus Dateiendung von OUTPUT geraten")
    parser.add_argument("--port", type=int, default=2002)
    parser.add_argument("--soffice", default=None, help="Pfad zu soffice(.exe)")
    parser.add_argument("--no-autostart", action="store_true",
                         help="Keine LibreOffice-Instanz automatisch starten")
    args = parser.parse_args()

    if uno is None:
        sys.exit(
            "FEHLER: Das 'uno'-Modul wurde nicht gefunden.\n"
            "Dieses Skript muss mit dem Python ausgefuehrt werden, das LibreOffice\n"
            "mitbringt, z.B. unter Windows:\n"
            r'  "C:\Program Files\LibreOffice\program\python.exe" fill_template.py ...'
        )

    if not os.path.exists(args.template):
        sys.exit(f"FEHLER: Vorlage nicht gefunden: {args.template}")
    if not os.path.exists(args.textfile):
        sys.exit(f"FEHLER: Textdatei nicht gefunden: {args.textfile}")

    out_fmt = guess_format(args.output, args.format)

    with open(args.textfile, "r", encoding="utf-8") as f:
        text_content = f.read()

    # Verbindung zu LibreOffice aufbauen (ggf. Instanz im Hintergrund starten)
    try:
        ctx = connect(args.port, retries=3, delay=0.5)
    except RuntimeError:
        if args.no_autostart:
            sys.exit(
                "FEHLER: Keine laufende LibreOffice-Instanz gefunden und "
                "--no-autostart gesetzt."
            )
        soffice_path = find_soffice_binary(args.soffice)
        start_listener(soffice_path, args.port)
        ctx = connect(args.port, retries=40, delay=1.0)

    desktop = get_desktop(ctx)

    load_props = (
        make_prop("Hidden", True),
        make_prop("ReadOnly", False),
    )
    doc = desktop.loadComponentFromURL(
        path_to_url(args.template), "_blank", 0, load_props
    )

    try:
        replace_placeholder_with_text(doc, args.placeholder, text_content)
        save_document(doc, args.output, out_fmt)
    finally:
        doc.close(False)  # Vorlage bleibt unveraendert, nur die Kopie wird gespeichert

    print(f"OK: '{args.output}' erstellt ({out_fmt}).")


if __name__ == "__main__":
    main()
