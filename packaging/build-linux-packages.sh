#!/bin/sh
# Baut .deb, .rpm und .AppImage aus einem fertigen Linux-Binary.
# Aufruf aus dem Projektordner: packaging/build-linux-packages.sh <binary> [ausgabeordner]
# Braucht Docker (für nfpm) und Netz (für appimagetool).
set -e
BIN=$1
OUT=${2:-dist}
[ -x "$BIN" ] || { echo "Binary fehlt: $BIN" >&2; exit 1; }
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
export VERSION
mkdir -p "$OUT" packaging/build
cp "$BIN" packaging/build/speedload

for p in deb rpm; do
  docker run --rm -e VERSION -v "$PWD":/work -w /work goreleaser/nfpm \
    package -f packaging/nfpm.yaml -p "$p" -t "$OUT/speedload-linux-x86_64.$p"
done

APPDIR=packaging/build/Speedload.AppDir
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin"
install -m 755 "$BIN" "$APPDIR/usr/bin/speedload"
cp packaging/speedload.desktop packaging/speedload.png "$APPDIR/"
printf '%s\n' '#!/bin/sh' 'exec "$APPDIR/usr/bin/speedload" "$@"' > "$APPDIR/AppRun"
chmod +x "$APPDIR/AppRun"
TOOL=packaging/build/appimagetool
[ -x "$TOOL" ] || { curl -fsSL -o "$TOOL" https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage; chmod +x "$TOOL"; }
ARCH=x86_64 "$TOOL" --appimage-extract-and-run "$APPDIR" "$OUT/speedload-linux-x86_64.AppImage"
ls -la "$OUT"
