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

APP_BIN="flash/esp32-2432s028-scrypt-miner.bin"
MERGED_BIN="flash/esp32-2432s028-scrypt-miner-merged.bin"
mkdir -p flash

echo "==> Building C++ CYD firmware (PlatformIO)..."
pio run -d "$ROOT/firmware-cpp" -e cyd

python3 - <<'PY'
from pathlib import Path
import sys
app = Path("flash/esp32-2432s028-scrypt-miner.bin").read_bytes()
merged = Path("flash/esp32-2432s028-scrypt-miner-merged.bin").read_bytes()
ok = True
if app[0] != 0xE9:
    print("ERROR: app.bin missing ESP magic 0xE9", file=sys.stderr); ok = False
if len(merged) < 0x10000 + 256:
    print("ERROR: merged.bin too small", file=sys.stderr); ok = False
elif merged[0x10000] != 0xE9:
    print("ERROR: merged.bin has no app at 0x10000", file=sys.stderr); ok = False
elif merged[0x1000] != 0xE9:
    print("ERROR: merged.bin has no bootloader at 0x1000", file=sys.stderr); ok = False
if app[:16] != merged[0x10000:0x10010]:
    print("ERROR: merged app segment != app.bin", file=sys.stderr); ok = False
print(f"Header OK: magic=0xE9 mode={app[2]:#04x} size/freq={app[3]:#04x}")
sys.exit(0 if ok else 1)
PY

echo "==> Writing SHA256SUMS.txt..."
(
  cd flash
  sha256sum esp32-2432s028-scrypt-miner.bin esp32-2432s028-scrypt-miner-merged.bin > SHA256SUMS.txt
  cat SHA256SUMS.txt
)

mkdir -p /opt/cursor/artifacts
cp -f "$APP_BIN" "$MERGED_BIN" flash/SHA256SUMS.txt /opt/cursor/artifacts/
ls -la "$APP_BIN" "$MERGED_BIN"
echo "Done. Flash merged.bin @ 0x0 (DIO, 4 MB, 40 MHz)."
