#!/usr/bin/env bash
# Build downloadable firmware + host packages under downloads/
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/downloads"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

mkdir -p "$OUT"

echo "==> Firmware zip"
FW_DIR="$STAGE/CYD-Firmware"
mkdir -p "$FW_DIR"
cp "$ROOT/firmware/release/esp32-cyd-pc-monitor-merged.bin" \
   "$ROOT/firmware/release/esp32-cyd-pc-monitor.bin" \
   "$ROOT/firmware/release/bootloader.bin" \
   "$ROOT/firmware/release/partitions.bin" \
   "$ROOT/firmware/release/FLASH.md" \
   "$FW_DIR/"
cat > "$FW_DIR/README.txt" <<'EOF'
CYD PC/GPU Monitor — firmware flash package
===========================================

Easiest flash (recommended):
  esptool.py --chip esp32 --port COMx --baud 921600 \
    write_flash 0x0 esp32-cyd-pc-monitor-merged.bin

Linux example:
  esptool.py --chip esp32 --port /dev/ttyUSB0 --baud 921600 \
    write_flash 0x0 esp32-cyd-pc-monitor-merged.bin

Install esptool if needed:
  pip install esptool

See FLASH.md for app-only / split flashing.
After flash, plug USB and run the host app from CYD-Monitor-portable.zip.
EOF
(
  cd "$STAGE"
  zip -qr "$OUT/CYD-Firmware.zip" CYD-Firmware
)

echo "==> Host portable zip"
if [[ -f "$ROOT/host/release/CYD-Monitor-portable.zip" ]]; then
  cp "$ROOT/host/release/CYD-Monitor-portable.zip" "$OUT/CYD-Monitor-portable.zip"
else
  echo "Missing host/release/CYD-Monitor-portable.zip — run host/build_app.sh first" >&2
  exit 1
fi

echo "==> Linux standalone binary"
if [[ -f "$ROOT/host/release/CYD-Monitor" ]]; then
  cp "$ROOT/host/release/CYD-Monitor" "$OUT/CYD-Monitor-linux"
  chmod +x "$OUT/CYD-Monitor-linux"
fi

echo "==> Full bundle (firmware + portable app)"
BUNDLE="$STAGE/CYD-Monitor-bundle"
mkdir -p "$BUNDLE/firmware" "$BUNDLE/app"
cp -a "$FW_DIR/." "$BUNDLE/firmware/"
unzip -q "$OUT/CYD-Monitor-portable.zip" -d "$BUNDLE/app"
cp "$ROOT/host/release/README.md" "$BUNDLE/APP-README.md"
cat > "$BUNDLE/START-HERE.txt" <<'EOF'
CYD PC/GPU Monitor — download bundle
====================================

1) Flash the display
   Folder: firmware/
   Flash:  esp32-cyd-pc-monitor-merged.bin  at address 0x0
   Details: firmware/FLASH.md  or  firmware/README.txt

2) Run the PC app
   Folder: app/CYD-Monitor/
   Windows: double-click "CYD Monitor.bat"
   Linux/macOS: ./CYD-Monitor.sh   (needs Python 3)
   Or use CYD-Monitor-linux from the downloads folder (Linux only)

3) Plug the CYD in over USB — the app auto-connects.

Screen flip / brightness: open the app → Settings → CYD display controls.
EOF
(
  cd "$STAGE"
  zip -qr "$OUT/CYD-Monitor-bundle.zip" CYD-Monitor-bundle
)

cat > "$OUT/README.md" <<'EOF'
# Downloads — CYD PC/GPU Monitor

Grab a zip, flash the board, run the app.

| Package | What it is |
| --- | --- |
| **[CYD-Monitor-bundle.zip](CYD-Monitor-bundle.zip)** | **Recommended** — firmware + portable PC app |
| **[CYD-Firmware.zip](CYD-Firmware.zip)** | Flash images only (`merged.bin` @ `0x0`) |
| **[CYD-Monitor-portable.zip](CYD-Monitor-portable.zip)** | PC app (Windows / Linux / macOS, needs Python) |
| **[CYD-Monitor-linux](CYD-Monitor-linux)** | Linux standalone binary (no Python) |

## Quick start

1. Unzip **CYD-Monitor-bundle.zip**
2. Flash `firmware/esp32-cyd-pc-monitor-merged.bin` at address `0x0`:

```bash
pip install esptool
esptool.py --chip esp32 --port COMx --baud 921600 \
  write_flash 0x0 esp32-cyd-pc-monitor-merged.bin
```

3. Run `app/CYD-Monitor/CYD Monitor.bat` (Windows) or `./CYD-Monitor.sh` (Linux/macOS)
4. Leave the CYD plugged in over USB

## GitHub direct links

On the feature branch (until merged):

- Bundle: `https://github.com/GutFarms/Japan-central/raw/cursor/esp32-cyd-pc-monitor-9f0c/downloads/CYD-Monitor-bundle.zip`
- Firmware: `https://github.com/GutFarms/Japan-central/raw/cursor/esp32-cyd-pc-monitor-9f0c/downloads/CYD-Firmware.zip`
- Portable app: `https://github.com/GutFarms/Japan-central/raw/cursor/esp32-cyd-pc-monitor-9f0c/downloads/CYD-Monitor-portable.zip`
EOF

# Also keep firmware zip mirrored next to bins for FLASH.md discoverability.
cp "$OUT/CYD-Firmware.zip" "$ROOT/firmware/release/CYD-Firmware.zip"

echo "Packaged:"
ls -lah "$OUT"
