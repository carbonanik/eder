#!/bin/sh
set -eu
source_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
desktop_file="$source_root/packaging/linux/io.github.carbonanik.eder.desktop"
metainfo="$source_root/packaging/linux/io.github.carbonanik.eder.metainfo.xml"
desktop-file-validate "$desktop_file"
appstreamcli validate --no-net --strict "$metainfo"
if [ "${1:-}" = --flathub ]; then
    flatpak run --command=flatpak-builder-lint org.flatpak.Builder appstream "$metainfo"
fi
