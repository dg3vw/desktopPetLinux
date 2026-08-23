#!/usr/bin/env bash
# Baut ein portables AppImage von desktop_sheep.
#
# Beim ersten Lauf werden linuxdeploy + linuxdeploy-plugin-gtk nach
# packaging/.cache/ heruntergeladen (~50 MB). Danach ist der Build offline.
#
# Ergebnis: packaging/build/Desktop_Sheep-<arch>.AppImage
#
# Hinweis zur glibc-Kompatibilität: Das AppImage bindet GTK4 + Cairo mit,
# aber NICHT die glibc. Wer maximale Portabilität will, baut auf dem
# ältesten unterstützten System (oder in einem Docker-Container mit z.B.
# Debian oldstable). Ansonsten läuft es problemlos auf Systemen mit
# gleich alter oder neuerer glibc als dem Build-Rechner.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CACHE_DIR="$SCRIPT_DIR/.cache"
BUILD_DIR="$SCRIPT_DIR/build"
APPDIR="$BUILD_DIR/AppDir"

ARCH="$(uname -m)"
export ARCH

LINUXDEPLOY="$CACHE_DIR/linuxdeploy-${ARCH}.AppImage"
PLUGIN_GTK="$CACHE_DIR/linuxdeploy-plugin-gtk.sh"

LINUXDEPLOY_URL="https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-${ARCH}.AppImage"
PLUGIN_GTK_URL="https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/master/linuxdeploy-plugin-gtk.sh"

mkdir -p "$CACHE_DIR" "$BUILD_DIR"

fetch_if_missing() {
    local url="$1" dest="$2"
    if [ ! -f "$dest" ]; then
        echo "==> Lade $(basename "$dest")"
        if command -v curl >/dev/null 2>&1; then
            curl -fL --retry 3 -o "$dest" "$url"
        elif command -v wget >/dev/null 2>&1; then
            wget -O "$dest" "$url"
        else
            echo "Weder curl noch wget gefunden – bitte installieren." >&2
            exit 1
        fi
        chmod +x "$dest"
    fi
}

fetch_if_missing "$LINUXDEPLOY_URL" "$LINUXDEPLOY"
fetch_if_missing "$PLUGIN_GTK_URL"  "$PLUGIN_GTK"

# linuxdeploy findet das gtk-Plugin über den PATH, wenn es dort als
# "linuxdeploy-plugin-gtk[.sh]" liegt.
export PATH="$CACHE_DIR:$PATH"

echo "==> cargo build --release"
(cd "$PROJECT_DIR" && cargo build --release)

BIN="$PROJECT_DIR/target/release/desktop_sheep"
if [ ! -x "$BIN" ]; then
    echo "Binary nicht gefunden: $BIN" >&2
    exit 1
fi

echo "==> AppDir aufbauen"
rm -rf "$APPDIR"
mkdir -p "$APPDIR"

# linuxdeploy schreibt die fertige .AppImage ins CWD.
cd "$BUILD_DIR"
"$LINUXDEPLOY" \
    --appdir "$APPDIR" \
    --executable "$BIN" \
    --desktop-file "$SCRIPT_DIR/desktop_sheep.desktop" \
    --icon-file "$SCRIPT_DIR/desktop_sheep.png" \
    --plugin gtk \
    --output appimage

echo
echo "==> Fertig:"
ls -lh "$BUILD_DIR"/*.AppImage 2>/dev/null || echo "  (keine .AppImage gefunden – siehe Log oben)"
