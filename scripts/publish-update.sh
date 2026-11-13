#!/usr/bin/env bash
# Publishes a release for the in-app updater: AppImage, .deb and latest.json go public into
# deldown-downloads/geffeniuse/. Without --yes it only prints what it would do.
#   scripts/publish-update.sh 0.3.1 [--yes] [--notes "Text"]
set -euo pipefail

VERSION="${1:?usage: publish-update.sh <version> [--yes] [--notes TEXT]}"; shift || true
YES=0; NOTES=""
while [ $# -gt 0 ]; do
  case "$1" in
    --yes) YES=1 ;;
    --notes) NOTES="${2:?--notes needs text}"; shift ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

DIR="${GEFFENIUSE_DIST:-$HOME/ssd}"
PROFILE="ovh-user"; BUCKET="deldown-downloads"; PREFIX="geffeniuse"
BASE="https://deldown-downloads.s3.de.io.cloud.ovh.net/$PREFIX"
APPIMAGE="$DIR/Geffeniuse-Launcher-$VERSION-x86_64.AppImage"
DEB="$DIR/geffeniuse_${VERSION}_amd64.deb"
WINZIP="$DIR/Geffeniuse-Launcher-$VERSION-windows-x64.zip"   # optional: only published when it exists
for f in "$APPIMAGE" "$DEB"; do [ -f "$f" ] || { echo "missing: $f" >&2; exit 1; }; done

SHA="$(sha256sum "$APPIMAGE" | cut -d' ' -f1)"
WINSHA=""; [ -f "$WINZIP" ] && WINSHA="$(sha256sum "$WINZIP" | cut -d' ' -f1)"
MANIFEST="$(mktemp)"; trap 'rm -f "$MANIFEST"' EXIT
python3 - "$VERSION" "$BASE" "$SHA" "$NOTES" "$WINSHA" > "$MANIFEST" <<'PY'
import json, sys
v, base, sha, notes, winsha = sys.argv[1:6]
m = {"version": v, "appimage_url": f"{base}/Geffeniuse-Launcher-{v}-x86_64.AppImage", "appimage_sha256": sha,
     "deb_url": f"{base}/geffeniuse_{v}_amd64.deb", "notes": notes or None}
if winsha:
    m["windows_url"] = f"{base}/Geffeniuse-Launcher-{v}-windows-x64.zip"
    m["windows_sha256"] = winsha
print(json.dumps(m, indent=2))
PY
cat "$MANIFEST"
if [ "$YES" != 1 ]; then echo "dry run: nothing uploaded. Add --yes to publish (public upload)."; exit 0; fi

# Files first, manifest last: no client ever sees a manifest that points at a missing file.
servertag s3 upload "$PROFILE" "$APPIMAGE" "$BUCKET" "$PREFIX/$(basename "$APPIMAGE")" --public
servertag s3 upload "$PROFILE" "$DEB" "$BUCKET" "$PREFIX/$(basename "$DEB")" --public
[ -n "$WINSHA" ] && servertag s3 upload "$PROFILE" "$WINZIP" "$BUCKET" "$PREFIX/$(basename "$WINZIP")" --public
servertag s3 upload "$PROFILE" "$MANIFEST" "$BUCKET" "$PREFIX/latest.json" --public

echo "verifying public download ..."
curl -fsS "$BASE/latest.json" | python3 -c 'import json,sys; print("manifest ok:", json.load(sys.stdin)["version"])'
GOT="$(curl -fsS "$BASE/$(basename "$APPIMAGE")" | sha256sum | cut -d' ' -f1)"
[ "$GOT" = "$SHA" ] && echo "AppImage checksum ok" || { echo "CHECKSUM MISMATCH" >&2; exit 1; }
if [ -n "$WINSHA" ]; then
  GOTW="$(curl -fsS "$BASE/$(basename "$WINZIP")" | sha256sum | cut -d' ' -f1)"
  [ "$GOTW" = "$WINSHA" ] && echo "Windows zip checksum ok" || { echo "WINDOWS CHECKSUM MISMATCH" >&2; exit 1; }
fi
