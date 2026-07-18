#!/usr/bin/env bash
#
# Baut Medivox.app aus dem Release-Binary.
#
# Ergebnis: target/Medivox.app -- ein Menueleisten-Agent (LSUIElement), den macOS als
# eigenstaendige App fuehrt. Damit bekommt der Shim einen stabilen Eintrag in den
# Bedienungshilfen (statt "Terminal" freizuschalten) und der Mikrofon-Dialog erscheint
# mit dem App-Namen.
#
# Nutzung:
#   ./make-app.sh
#
# Danach die App einmal starten (open target/Medivox.app), in
#   Systemeinstellungen > Datenschutz & Sicherheit > Bedienungshilfen
# freischalten und neu starten.

set -euo pipefail

cd "$(dirname "$0")"

APP_NAME="Medivox"
BIN_NAME="medivox-shim"
BUNDLE="target/${APP_NAME}.app"

echo "==> cargo build --release"
cargo build --release

echo "==> Bundle-Struktur anlegen: ${BUNDLE}"
rm -rf "${BUNDLE}"
mkdir -p "${BUNDLE}/Contents/MacOS"
mkdir -p "${BUNDLE}/Contents/Resources"

cp "bundle/Info.plist" "${BUNDLE}/Contents/Info.plist"
cp "target/release/${BIN_NAME}" "${BUNDLE}/Contents/MacOS/${BIN_NAME}"

# Ad-hoc-Signatur (Signatur "-"): kein Entwicklerzertifikat noetig. macOS bindet die
# Bedienungshilfen-/Mikrofon-Freigabe an die Code-Identitaet -- ohne Signatur muesste sie
# nach jedem Rebuild neu erteilt werden.
echo "==> Ad-hoc codesign"
codesign --force --sign - "${BUNDLE}"

echo "==> Fertig: ${BUNDLE}"
echo "    Start: open \"${BUNDLE}\""
