#!/usr/bin/env bash
# Copies the public part of the launcher into the GitHub checkout, scans it for secrets and pushes.
# Without --yes it only shows what would change.
#   scripts/sync-github.sh "Commit message" [--yes]
set -euo pipefail
MESSAGE="${1:?usage: sync-github.sh \"commit message\" [--yes]}"
YES=0; [ "${2:-}" = "--yes" ] && YES=1
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PUBLIC="${GEFFENIUSE_PUBLIC:-$HOME/ssd/geffeniuse-launcher-public}"
REPO="https://github.com/deldown/geffeniuse-launcher.git"

[ -d "$PUBLIC/.git" ] || git clone -q "$REPO" "$PUBLIC"
git -C "$PUBLIC" pull -q --ff-only

cd "$ROOT"
rsync -a --delete --exclude .git --exclude target --exclude 'installer/payload/*' --exclude 'ui/art/ver-*.webp' \
  --exclude '*.log' --exclude 'static_analysis_codeql_*' --exclude 'src-tauri/gen' \
  crates src-tauri ui installer scripts tests assets Cargo.toml Cargo.lock LICENSE SECURITY.md THIRD-PARTY-LICENSES.md \
  README.md .gitignore "$PUBLIC/"
mkdir -p "$PUBLIC/installer/payload"

cd "$PUBLIC"
git add -A
if git diff --cached --quiet; then echo "nothing to publish"; exit 0; fi
git diff --cached --stat | tail -15

if git diff --cached -U0 | grep -nE '^\+.*(/home/[a-z]|BEGIN [A-Z ]*PRIVATE KEY|AKIA[0-9A-Z]{16}|gh[pousr]_[A-Za-z0-9]{20,})'; then
  echo "blocked: private path or key material in the change" >&2; git reset -q; exit 1
fi
# Discord application ids are public by design; the only hit is a made-up id in a test.
FOUND="$(semgrep scan --config=p/secrets --config=p/gitleaks --metrics=off --disable-version-check \
  --exclude-rule generic.secrets.gitleaks.discord-client-id.discord-client-id --json . 2>/dev/null \
  | python3 -c 'import json,sys; d=json.load(sys.stdin); print(len(d["results"])+len(d["errors"]))')"
if [ "$FOUND" != 0 ]; then echo "blocked: semgrep secret scan reports $FOUND findings or errors" >&2; git reset -q; exit 1; fi
echo "secret scan: clean"

if [ "$YES" != 1 ]; then git reset -q; echo "dry run: nothing committed. Add --yes to push."; exit 0; fi
git -c user.name=deldown -c user.email=161086438+deldown@users.noreply.github.com commit -q -m "$MESSAGE"
git push -q origin main
echo "pushed $(git rev-parse --short HEAD) to $REPO"
