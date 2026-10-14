#!/usr/bin/env bash
# Installs Beholder for the current user: binary, app icon, and desktop entry.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_SRC="$REPO_ROOT/target/release/beholder"
# Inside a release archive the binary sits at the archive root.
[ -f "$REPO_ROOT/beholder" ] && BIN_SRC="$REPO_ROOT/beholder"
BIN_DEST="$HOME/.local/bin/beholder"
ICON_DEST_DIR="$HOME/.local/share/icons/hicolor"
DESKTOP_DEST="$HOME/.local/share/applications/beholder.desktop"

if [ ! -f "$BIN_SRC" ]; then
    echo "Binary not found at $BIN_SRC. Build it first with: cargo build --release" >&2
    exit 1
fi

mkdir -p "$(dirname "$BIN_DEST")"
cp "$BIN_SRC" "$BIN_DEST"
chmod +x "$BIN_DEST"

for size in 16 32 48 64 128 256 512; do
    mkdir -p "$ICON_DEST_DIR/${size}x${size}/apps"
    cp "$REPO_ROOT/assets/icons/beholder-${size}.png" "$ICON_DEST_DIR/${size}x${size}/apps/beholder.png"
done

mkdir -p "$(dirname "$DESKTOP_DEST")"
cp "$REPO_ROOT/packaging/beholder.desktop" "$DESKTOP_DEST"

command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -f "$ICON_DEST_DIR" || true
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$HOME/.local/share/applications" || true

echo "Beholder installed. Make sure $HOME/.local/bin is on your PATH."
