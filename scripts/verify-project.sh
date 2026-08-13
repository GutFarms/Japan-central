#!/usr/bin/env bash
# Build + verify C++ firmware and Windows companion
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "==> SHA256d host verification (Bitcoin genesis)"
python3 ./scripts/verify-sha256.py

echo "==> stratum header endianness + LE nonce submit format"
python3 ./scripts/verify-stratum-header.py

echo "==> multi-UART cmp ping (two fake boards; second while first held)"
python3 ./scripts/verify-multi-usb-scan.py

echo "==> firmware + flash images"
./scripts/build-flash-images.sh

echo "==> Windows companion"
./scripts/build-companion-windows.sh

APP_BIN="flash/esp32-2432s028-sha256-miner.bin"
MERGED_BIN="flash/esp32-2432s028-sha256-miner-merged.bin"
ZIP="dist/cyd-companion-windows.zip"
PORTABLE_ZIP="dist/CYD-Companion-Portable.zip"
APP_ONLY_ZIP="dist/CYD-Companion-App-Only.zip"
KIT_ZIP="dist/CYD-Miner-Portable.zip"
EXE="dist/cyd-companion-windows/cyd-companion.exe"
SETUP="dist/CYD-Miner-Setup.exe"
SETUP_ALIAS="dist/CYD-Companion-Setup.exe"

echo "==> verifying artifacts"
test -f "$APP_BIN"
test -f "$MERGED_BIN"
test -f "$ZIP"
test -f "$PORTABLE_ZIP"
test -f "$APP_ONLY_ZIP"
test -f "$KIT_ZIP"
test -f "$EXE"
if [[ -f "$SETUP" ]]; then
  test -f "$SETUP"
  test -f "$SETUP_ALIAS"
fi

python3 - <<'PY'
from pathlib import Path
import sys
app = Path("flash/esp32-2432s028-sha256-miner.bin").read_bytes()
merged = Path("flash/esp32-2432s028-sha256-miner-merged.bin").read_bytes()
dl = Path("flash/downloads/esp32-2432s028-sha256-miner-merged.bin")
ok = True
if app[0] != 0xE9:
    print("ERROR: app.bin missing ESP magic 0xE9", file=sys.stderr); ok = False
if len(merged) < 0x10000 + 256:
    print("ERROR: merged.bin too small", file=sys.stderr); ok = False
elif merged[0x10000] != 0xE9 or merged[0x1000] != 0xE9:
    print("ERROR: merged.bin layout bad", file=sys.stderr); ok = False
elif app[:16] != merged[0x10000:0x10010]:
    print("ERROR: merged app segment != app.bin", file=sys.stderr); ok = False
if not dl.is_file() or dl.read_bytes() != merged:
    print("ERROR: flash/downloads merged.bin != flash/merged.bin", file=sys.stderr); ok = False
ver = Path("flash/downloads/VERSION.txt").read_text().strip()
cargo = next(
    (ln.split('"')[1] for ln in Path("companion/Cargo.toml").read_text().splitlines() if ln.startswith("version")),
    "",
)
if not ver.startswith(cargo):
    print(f"ERROR: downloads VERSION {ver!r} != Cargo {cargo!r}", file=sys.stderr); ok = False
sys.exit(0 if ok else 1)
PY

mkdir -p /opt/cursor/artifacts
cp -f "$MERGED_BIN" /opt/cursor/artifacts/
cp -f "$ZIP" /opt/cursor/artifacts/
cp -f "$PORTABLE_ZIP" /opt/cursor/artifacts/
cp -f "$APP_ONLY_ZIP" /opt/cursor/artifacts/
cp -f "$KIT_ZIP" /opt/cursor/artifacts/
[[ -f "$SETUP" ]] && cp -f "$SETUP" /opt/cursor/artifacts/
[[ -f "$SETUP_ALIAS" ]] && cp -f "$SETUP_ALIAS" /opt/cursor/artifacts/
cp -f flash/SHA256SUMS.txt /opt/cursor/artifacts/esp32-2432s028-SHA256SUMS.txt

echo
echo "OK — SHA-256 firmware + companion verified"
ls -la "$MERGED_BIN" "$EXE" "$PORTABLE_ZIP" "$APP_ONLY_ZIP" "$KIT_ZIP"
[[ -f "$SETUP" ]] && ls -la "$SETUP" "$SETUP_ALIAS"
sha256sum "$MERGED_BIN" "$PORTABLE_ZIP" "$APP_ONLY_ZIP" "$KIT_ZIP"
[[ -f "$SETUP" ]] && sha256sum "$SETUP"
cat flash/SHA256SUMS.txt
