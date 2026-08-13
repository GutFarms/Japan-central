#!/usr/bin/env bash
# Build C++ CYD firmware → flashable .bin images in ./flash/
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

export PATH="${HOME}/.local/bin:${PATH}"
if ! command -v pio >/dev/null 2>&1; then
  echo "ERROR: PlatformIO (pio) not found" >&2
  exit 1
fi

APP_BIN="flash/esp32-2432s028-sha256-miner.bin"
MERGED_BIN="flash/esp32-2432s028-sha256-miner-merged.bin"
APP_D0="flash/esp32-2432s028-sha256-miner-d0.bin"
MERGED_D0="flash/esp32-2432s028-sha256-miner-d0-merged.bin"
mkdir -p flash

echo "==> Building C++ CYD D0 firmware (PlatformIO env cyd-d0)..."
pio run -d "$ROOT/firmware-cpp" -e cyd-d0

python3 - <<'PY'
from pathlib import Path
import sys
ok = True
for app_path, merged_path in [
    ("flash/esp32-2432s028-sha256-miner.bin", "flash/esp32-2432s028-sha256-miner-merged.bin"),
    ("flash/esp32-2432s028-sha256-miner-d0.bin", "flash/esp32-2432s028-sha256-miner-d0-merged.bin"),
]:
    app = Path(app_path).read_bytes()
    merged = Path(merged_path).read_bytes()
    if app[0] != 0xE9:
        print(f"ERROR: {app_path} missing ESP magic 0xE9", file=sys.stderr); ok = False
    if len(merged) < 0x10000 + 256:
        print(f"ERROR: {merged_path} too small", file=sys.stderr); ok = False
    elif merged[0x10000] != 0xE9:
        print(f"ERROR: {merged_path} has no app at 0x10000", file=sys.stderr); ok = False
    elif merged[0x1000] != 0xE9:
        print(f"ERROR: {merged_path} has no bootloader at 0x1000", file=sys.stderr); ok = False
    if app[:16] != merged[0x10000:0x10010]:
        print(f"ERROR: {merged_path} app segment != app.bin", file=sys.stderr); ok = False
    print(f"Header OK ({Path(app_path).name}): magic=0xE9 mode={app[2]:#04x} size/freq={app[3]:#04x}")
sys.exit(0 if ok else 1)
PY

echo "==> Writing SHA256SUMS.txt..."
(
  cd flash
  sha256sum \
    esp32-2432s028-sha256-miner.bin \
    esp32-2432s028-sha256-miner-merged.bin \
    esp32-2432s028-sha256-miner-d0.bin \
    esp32-2432s028-sha256-miner-d0-merged.bin \
    > SHA256SUMS.txt
  cat SHA256SUMS.txt
)

mkdir -p /opt/cursor/artifacts flash/downloads
cp -f "$APP_BIN" "$MERGED_BIN" "$APP_D0" "$MERGED_D0" flash/SHA256SUMS.txt /opt/cursor/artifacts/
# Keep raw GitHub download URLs current for Companion "Fetch latest FW".
cp -f "$APP_BIN" "$MERGED_BIN" "$APP_D0" "$MERGED_D0" flash/SHA256SUMS.txt flash/downloads/
ls -la "$APP_BIN" "$MERGED_BIN" "$APP_D0" "$MERGED_D0"
echo "Done. Flash *-merged.bin @ 0x0 (DIO, 4 MB, 40 MHz). Prefer *-d0-merged.bin for ESP32-D0 auto-tune."
