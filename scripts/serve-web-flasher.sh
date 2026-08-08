#!/usr/bin/env bash
# Serve the drag-and-drop web flasher (needs HTTP for Web Serial + bundled .bin).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ ! -f flash/esp32-2432s028-scrypt-miner-merged.bin ]]; then
  echo "No merged.bin yet — building flash images..."
  "$ROOT/scripts/build-flash-images.sh"
fi

PORT="${1:-8080}"
echo ""
echo "  Open in Chrome or Edge:"
echo "    http://127.0.0.1:${PORT}/web/"
echo ""
echo "  Then drag flash/esp32-2432s028-scrypt-miner-merged.bin onto the page"
echo "  (or click “Load project merged.bin”)."
echo ""
cd flash
exec python3 -m http.server "$PORT" --bind 127.0.0.1
