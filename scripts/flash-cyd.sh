#!/usr/bin/env bash
# Flash merged C++ firmware to ESP32-2432S028
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PORT="${1:-}"
MERGED="$ROOT/flash/esp32-2432s028-scrypt-miner-merged.bin"

if [[ ! -f "$MERGED" ]]; then
  echo "Building firmware first..."
  "$ROOT/scripts/build-flash-images.sh"
fi

if [[ -z "$PORT" ]]; then
  echo "Usage: $0 <COM port or /dev/ttyUSB0>" >&2
  exit 1
fi

if command -v espflash >/dev/null 2>&1; then
  espflash write-bin -p "$PORT" 0x0 "$MERGED"
elif [[ -f "$HOME/.platformio/packages/tool-esptoolpy/esptool.py" ]]; then
  python3 "$HOME/.platformio/packages/tool-esptoolpy/esptool.py" \
    --chip esp32 -p "$PORT" write_flash -z 0x0 "$MERGED"
else
  echo "ERROR: need espflash or PlatformIO esptool.py" >&2
  exit 1
fi
