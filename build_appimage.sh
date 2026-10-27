#!/bin/sh
set -eu

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
APPIMAGETOOL="${APPIMAGETOOL:-appimagetool}"

if ! command -v "$APPIMAGETOOL" >/dev/null 2>&1; then
    APPIMAGETOOL=/tmp/appimagetool-x86_64.AppImage
fi

if ! command -v "$APPIMAGETOOL" >/dev/null 2>&1; then
    sh "$ROOT/packaging/appimage/download-tool.sh"
    APPIMAGETOOL=/tmp/appimagetool-x86_64.AppImage
fi

export APPIMAGETOOL
exec "$ROOT/packaging/appimage/build.sh"
