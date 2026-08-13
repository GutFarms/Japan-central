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
APP_D0="flash/esp32-2432s028-sha256-miner-d0.bin"
MERGED_D0="flash/esp32-2432s028-sha256-miner-d0-merged.bin"
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
test -f "$APP_D0"
test -f "$MERGED_D0"
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
ok = True
for app_path, merged_path, dl_path in [
    (
        "flash/esp32-2432s028-sha256-miner.bin",
        "flash/esp32-2432s028-sha256-miner-merged.bin",
        "flash/downloads/esp32-2432s028-sha256-miner-merged.bin",
    ),
    (
        "flash/esp32-2432s028-sha256-miner-d0.bin",
        "flash/esp32-2432s028-sha256-miner-d0-merged.bin",
        "flash/downloads/esp32-2432s028-sha256-miner-d0-merged.bin",
    ),
]:
    app = Path(app_path).read_bytes()
    merged = Path(merged_path).read_bytes()
    dl = Path(dl_path)
    if app[0] != 0xE9:
        print(f"ERROR: {app_path} missing ESP magic 0xE9", file=sys.stderr); ok = False
    if len(merged) < 0x10000 + 256:
        print(f"ERROR: {merged_path} too small", file=sys.stderr); ok = False
    elif merged[0x10000] != 0xE9 or merged[0x1000] != 0xE9:
        print(f"ERROR: {merged_path} layout bad", file=sys.stderr); ok = False
    elif app[:16] != merged[0x10000:0x10010]:
        print(f"ERROR: {merged_path} app segment != app.bin", file=sys.stderr); ok = False
    if not dl.is_file() or dl.read_bytes() != merged:
        print(f"ERROR: downloads {dl.name} != flash/{Path(merged_path).name}", file=sys.stderr); ok = False
ver = Path("flash/downloads/VERSION.txt").read_text().strip()
cargo = next(
    (ln.split('"')[1] for ln in Path("companion/Cargo.toml").read_text().splitlines() if ln.startswith("version")),
    "",
)
if not ver.startswith(cargo):
    print(f"ERROR: downloads VERSION {ver!r} != Cargo {cargo!r}", file=sys.stderr); ok = False
sums = Path("flash/downloads/SHA256SUMS.txt")
if not sums.is_file():
    print("ERROR: missing flash/downloads/SHA256SUMS.txt", file=sys.stderr); ok = False
else:
    import hashlib, re
    text = sums.read_text()
    entries = {}
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split()
        if len(parts) < 2 or len(parts[0]) != 64:
            continue
        name = parts[1].lstrip("*").split("/")[-1]
        entries[name] = parts[0].lower()
    required = [
        "cyd-companion.exe",
        "CYD-Companion-App-Only.zip",
        "CYD-Miner-Portable.zip",
        "esp32-2432s028-sha256-miner-merged.bin",
    ]
    for name in required:
        path = Path("flash/downloads") / name
        if not path.is_file():
            print(f"ERROR: missing download {name}", file=sys.stderr); ok = False
            continue
        if name not in entries:
            print(f"ERROR: SHA256SUMS missing {name}", file=sys.stderr); ok = False
            continue
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if digest != entries[name]:
            print(f"ERROR: hash mismatch {name}", file=sys.stderr); ok = False
    # PE VERSIONINFO / publisher string should be present on the Windows exe.
    exe = Path("flash/downloads/cyd-companion.exe")
    if exe.is_file():
        blob = exe.read_bytes()
        gut_utf16 = "GutFarms".encode("utf-16le")
        if b"GutFarms" not in blob and gut_utf16 not in blob:
            print("ERROR: cyd-companion.exe missing GutFarms VERSIONINFO", file=sys.stderr)
            ok = False
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
cp -f flash/downloads/SHA256SUMS.txt /opt/cursor/artifacts/DOWNLOAD_SHA256SUMS.txt
cp -f flash/downloads/cyd-companion.exe /opt/cursor/artifacts/ 2>/dev/null || true

echo
echo "OK — SHA-256 firmware + companion verified (incl. download SHA256SUMS + PE metadata)"
ls -la "$MERGED_BIN" "$EXE" "$PORTABLE_ZIP" "$APP_ONLY_ZIP" "$KIT_ZIP"
[[ -f "$SETUP" ]] && ls -la "$SETUP" "$SETUP_ALIAS"
sha256sum "$MERGED_BIN" "$PORTABLE_ZIP" "$APP_ONLY_ZIP" "$KIT_ZIP"
[[ -f "$SETUP" ]] && sha256sum "$SETUP"
echo "--- flash/downloads/SHA256SUMS.txt ---"
cat flash/downloads/SHA256SUMS.txt
echo "--- flash/SHA256SUMS.txt (firmware) ---"
cat flash/SHA256SUMS.txt
