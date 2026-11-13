#!/usr/bin/env bash
# Builds the Windows installer around the already cross-built launcher (run winbuild/build.sh first).
#   scripts/build-installer.sh 0.5.4   ->  ~/ssd/Geffeniuse-Launcher-0.5.4-Setup.exe
set -euo pipefail
VERSION="${1:?usage: build-installer.sh <version>}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${GEFFENIUSE_DIST:-$HOME/ssd}"
WINBUILD="${GEFFENIUSE_WINBUILD:-$HOME/ssd/winbuild}"
export PATH="$WINBUILD/zigenv/bin:$WINBUILD/cargo-root/bin:$HOME/.cargo/bin:$PATH"
export TMPDIR="${GEFFENIUSE_TMP:-$HOME/ssd/tmp}" CARGO_BUILD_JOBS=1 GEFFENIUSE_VERSION="$VERSION"
export CARGO_TARGET_DIR="$WINBUILD/installer-target"
cp "$ROOT/target/x86_64-pc-windows-gnu/release/geffeniuse.exe" "$ROOT/installer/payload/geffeniuse.exe"
cp "$(ls ~/.cargo/registry/src/*/webview2-com-sys-*/x64/WebView2Loader.dll | tail -1)" "$ROOT/installer/payload/WebView2Loader.dll"
cd "$ROOT/installer"
python -m ziglang rc /fo payload/icon.res icon.rc 2>/dev/null || "$WINBUILD/zigenv/bin/python" -m ziglang rc /fo payload/icon.res icon.rc
$(command -v build-serial || true) cargo zigbuild --release --target x86_64-pc-windows-gnu
cp "$CARGO_TARGET_DIR/x86_64-pc-windows-gnu/release/geffeniuse-installer.exe" "$OUT/Geffeniuse-Launcher-$VERSION-Setup.exe"
sha256sum "$OUT/Geffeniuse-Launcher-$VERSION-Setup.exe"
