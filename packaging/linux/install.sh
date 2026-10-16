#!/bin/sh
# Installs Stage-Lite for the current user: the binary in ~/.local/bin and a
# menu entry. Run from the unpacked release folder: ./install.sh
set -e
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications"
install -m 755 "$here/stage-lite" "$HOME/.local/bin/stage-lite"
sed "s|^Exec=.*|Exec=$HOME/.local/bin/stage-lite|" "$here/stage-lite.desktop" \
  > "$HOME/.local/share/applications/stage-lite.desktop"
echo "Installed. Start it from the menu or run: $HOME/.local/bin/stage-lite"
