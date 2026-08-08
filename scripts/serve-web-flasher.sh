#!/usr/bin/env bash
# Serve the drag-and-drop web flasher (needs HTTP for Web Serial + .bin download).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ ! -f flash/esp32-2432s028-scrypt-miner-merged.bin ]]; then
  echo "No merged.bin yet — building flash images..."
  "$ROOT/scripts/build-flash-images.sh"
fi

PORT="${1:-8080}"
SIZE="$(wc -c < flash/esp32-2432s028-scrypt-miner-merged.bin | tr -d ' ')"
echo ""
echo "  CYD web flasher"
echo "  ---------------"
echo "  Open in Chrome or Edge:"
echo "    http://127.0.0.1:${PORT}/web/"
echo ""
echo "  1) Click  Save merged.bin to PC"
echo "  2) (optional) drag it back onto the page"
echo "  3) Connect & flash  →  pick your COM / tty port"
echo ""
echo "  merged.bin size: ${SIZE} bytes"
if [[ -f flash/SHA256SUMS.txt ]]; then
  echo "  checksums: flash/SHA256SUMS.txt"
fi
echo ""
cd flash
exec python3 -m http.server "$PORT" --bind 127.0.0.1
