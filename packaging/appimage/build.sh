#!/bin/sh
set -eu

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
cd "$ROOT"

if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo is required to build Folio." >&2
    exit 1
fi

APPIMAGETOOL="${APPIMAGETOOL:-appimagetool}"
if ! command -v "$APPIMAGETOOL" >/dev/null 2>&1; then
    echo "appimagetool is required; set APPIMAGETOOL to its executable path." >&2
    exit 1
fi

cargo build --locked --release

VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)"
ARCH="$(uname -m)"
OUTPUT="$ROOT/target/appimage/folio-reader-$VERSION-$ARCH.AppImage"
APPDIR="$(mktemp -d "${TMPDIR:-/tmp}/folio-appimage.XXXXXX")"
trap 'rm -rf "$APPDIR"' EXIT HUP INT TERM

mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/scalable/apps" "$ROOT/target/appimage"
install -m 755 target/release/folio-reader "$APPDIR/usr/bin/folio-reader"
install -m 755 packaging/appimage/AppRun "$APPDIR/AppRun"
install -m 644 packaging/appimage/folio-reader.desktop "$APPDIR/folio-reader.desktop"
install -m 644 packaging/appimage/folio-reader.desktop "$APPDIR/usr/share/applications/folio-reader.desktop"
install -m 644 packaging/appimage/folio-reader.svg "$APPDIR/folio-reader.svg"
install -m 644 packaging/appimage/folio-reader.svg "$APPDIR/usr/share/icons/hicolor/scalable/apps/folio-reader.svg"

if [ "${APPIMAGETOOL##*.}" = AppImage ]; then
    "$APPIMAGETOOL" --appimage-extract-and-run "$APPDIR" "$OUTPUT"
else
    "$APPIMAGETOOL" "$APPDIR" "$OUTPUT"
fi

echo "Built $OUTPUT"
