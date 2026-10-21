#!/bin/sh
# Install an already-built executable and upstream desktop metadata.
set -eu

source_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
prefix=${PREFIX:-/usr/local}
destination=${DESTDIR:-}${prefix}
binary=${1:-"$source_root/target/release/eder"}
app_id=io.github.carbonanik.eder

install -Dm755 "$binary" "$destination/bin/eder"
install -Dm644 "$source_root/packaging/linux/$app_id.desktop" \
    "$destination/share/applications/$app_id.desktop"
install -Dm644 "$source_root/packaging/linux/$app_id.metainfo.xml" \
    "$destination/share/metainfo/$app_id.metainfo.xml"
install -Dm644 "$source_root/packaging/appimage/eder.svg" \
    "$destination/share/icons/hicolor/scalable/apps/$app_id.svg"
install -Dm644 "$source_root/LICENSE" \
    "$destination/share/licenses/$app_id/LICENSE"
install -Dm644 "$source_root/THIRD_PARTY.md" \
    "$destination/share/licenses/$app_id/THIRD_PARTY.md"
mkdir -p "$destination/share/licenses/$app_id/THIRD_PARTY"
cp -R "$source_root/THIRD_PARTY/." "$destination/share/licenses/$app_id/THIRD_PARTY/"
