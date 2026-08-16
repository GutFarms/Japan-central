#!/usr/bin/env bash
# Wipe local build caches / scratch outputs. Does not touch flash/downloads/.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "==> cleaning PlatformIO (.pio)"
rm -rf firmware-cpp/.pio
if command -v pio >/dev/null 2>&1; then
  (cd firmware-cpp && pio run -t clean >/dev/null 2>&1 || true)
fi

echo "==> cleaning Companion Rust target"
rm -rf companion/target companion/build
if command -v cargo >/dev/null 2>&1; then
  (cd companion && cargo clean >/dev/null 2>&1 || true)
fi

echo "==> cleaning dist/ + scratch flash merges"
rm -rf dist
rm -f flash/_tmp-merged.bin flash/_tmp-*.bin

echo "==> cleaning mobile caches (if present)"
rm -rf mobile/node_modules mobile/.expo mobile/dist

echo "==> cleaning misc junk"
find . -name '*.rs.bk' -delete 2>/dev/null || true
find . -name '.DS_Store' -delete 2>/dev/null || true
rm -rf ApiDownloads **/ApiDownloads 2>/dev/null || true

echo "Done. flash/downloads/ kept (shipped artifacts)."
