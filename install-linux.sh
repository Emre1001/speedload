#!/bin/sh
# Baut Speedload, installiert nach ~/.local/bin, legt Menüeintrag und Autostart (unsichtbar) an.
set -e
cd "$(dirname "$0")"
cargo build --release
# Erweiterung als .xpi für Firefox ESR / Mullvad Browser
(cd extension && python3 -m zipfile -c ../speedload.xpi manifest.json background.js)
mkdir -p ~/.local/bin ~/.local/share/applications ~/.config/autostart
install -m 755 target/release/speedload ~/.local/bin/speedload
cat > ~/.local/share/applications/speedload.desktop <<EOF
[Desktop Entry]
Type=Application
Name=Speedload
Comment=Schneller Download-Manager
Exec=$HOME/.local/bin/speedload
Icon=folder-download
Categories=Network;FileTransfer;
EOF
sed 's|Exec=.*|Exec='"$HOME"'/.local/bin/speedload --hidden|' ~/.local/share/applications/speedload.desktop > ~/.config/autostart/speedload.desktop
echo "Fertig. Start: Menü > Speedload"
