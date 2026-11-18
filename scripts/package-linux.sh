#!/usr/bin/env bash
# Builds the .deb and the AppImage from target/release/geffeniuse into ~/ssd (or $GEFFENIUSE_DIST).
# The program inside the packages is called geffeniuse-launcher, not geffeniuse: some desktops (KDE) also match a
# running window to a menu entry by the name of the executable, and "geffeniuse" belongs to the old launcher.
# The desktop file and icon carry the app's identifier, because that is the id the window reports to
# the taskbar; with another name the shell cannot match them and shows a generic or an old icon.
#   scripts/package-linux.sh 0.6.3
set -euo pipefail
# Package contents must be world-readable whatever umask the calling shell uses.
umask 022
V="${1:?usage: package-linux.sh <version>}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; OUT="${GEFFENIUSE_DIST:-$HOME/ssd}"; TMP="${TMPDIR:-/tmp}"
ID=de.deldown.geffeniuse
DESKTOP="[Desktop Entry]
Type=Application
Name=Geffeniuse Launcher
Comment=Minecraft launcher with modpacks
Exec=geffeniuse-launcher
Icon=$ID
StartupWMClass=$ID
Categories=Game;
Terminal=false
"
R="$TMP/debroot"; rm -rf "$R"
mkdir -p "$R/DEBIAN" "$R/usr/bin" "$R/usr/share/applications" "$R/usr/share/icons/hicolor/64x64/apps" "$R/usr/share/icons/hicolor/256x256/apps"
install -m755 "$ROOT/target/release/geffeniuse" "$R/usr/bin/geffeniuse-launcher"
install -m644 "$ROOT/assets/logo-64.png" "$R/usr/share/icons/hicolor/64x64/apps/$ID.png"
install -m644 "$ROOT/assets/logo-256.png" "$R/usr/share/icons/hicolor/256x256/apps/$ID.png"
printf '%s' "$DESKTOP" > "$R/usr/share/applications/$ID.desktop"
printf 'Package: geffeniuse\nVersion: %s\nSection: games\nPriority: optional\nArchitecture: amd64\nDepends: libwebkit2gtk-4.1-0, libgtk-3-0\nInstalled-Size: %s\nMaintainer: deldown <claude@deldown.de>\nDescription: Geffeniuse Launcher\n Minecraft launcher with modpacks.\n' "$V" "$(du -sk "$R/usr" | cut -f1)" > "$R/DEBIAN/control"
dpkg-deb --root-owner-group --build "$R" "$OUT/geffeniuse_${V}_amd64.deb" | tail -1
T="$TMP/aitool"; rm -rf "$T"; mkdir -p "$T"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/geffeniuse-build/appimagetool"
if [ ! -f "$CACHE" ]; then
  mkdir -p "$(dirname "$CACHE")"
  curl -fLsS --max-time 300 -o "$CACHE.part" https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
  mv "$CACHE.part" "$CACHE"
fi
cp "$CACHE" "$T/appimagetool"
echo "e8c1169c4af66c3184d7ee110713d4be04befe671fa358e2c1591ee840553b03  $T/appimagetool" | sha256sum -c - >/dev/null
chmod +x "$T/appimagetool"
A="$TMP/Geffeniuse.AppDir"; rm -rf "$A"; mkdir -p "$A/usr/bin"
install -m755 "$ROOT/target/release/geffeniuse" "$A/usr/bin/geffeniuse-launcher"
cp "$ROOT/assets/logo-256.png" "$A/$ID.png"; cp "$ROOT/assets/logo-256.png" "$A/.DirIcon"
printf '%s' "$DESKTOP" > "$A/$ID.desktop"
printf '#!/bin/sh\nHERE="$(dirname "$(readlink -f "$0")")"\nexec "$HERE/usr/bin/geffeniuse-launcher" "$@"\n' > "$A/AppRun"; chmod +x "$A/AppRun"
ARCH=x86_64 "$T/appimagetool" --appimage-extract-and-run "$A" "$OUT/Geffeniuse-Launcher-${V}-x86_64.AppImage" >/dev/null 2>&1
rm -rf "$A" "$T"; ls -la "$OUT/Geffeniuse-Launcher-${V}-x86_64.AppImage" "$OUT/geffeniuse_${V}_amd64.deb" | awk '{print $5, $9}'
