#!/usr/bin/env bash
# Build a standalone CYD Monitor executable with PyInstaller.
set -euo pipefail
cd "$(dirname "$0")"

python3 -m venv .venv-build
# shellcheck disable=SC1091
source .venv-build/bin/activate
pip install -q --upgrade pip
pip install -q -r requirements.txt pyinstaller

rm -rf build dist
pyinstaller \
  --noconfirm \
  --clean \
  --onefile \
  --windowed \
  --name "CYD-Monitor" \
  --hidden-import serial \
  --hidden-import serial.tools.list_ports \
  --hidden-import pynvml \
  --hidden-import cyd_core \
  --hidden-import pystray \
  --hidden-import PIL \
  --hidden-import PIL.Image \
  --hidden-import PIL.ImageDraw \
  --collect-all pystray \
  desktop_app.py

mkdir -p release
if [[ -f dist/CYD-Monitor ]]; then
  cp dist/CYD-Monitor release/CYD-Monitor
  chmod +x release/CYD-Monitor
elif [[ -f dist/CYD-Monitor.exe ]]; then
  cp dist/CYD-Monitor.exe release/CYD-Monitor.exe
fi

# Portable zip (source + launchers) for machines with Python.
STAGE="$(mktemp -d)"
PKG="$STAGE/CYD-Monitor"
mkdir -p "$PKG"
cp cyd_core.py agent.py desktop_app.py simulate_demo.py requirements.txt \
  "CYD Monitor.bat" CYD-Monitor.sh "$PKG/"
chmod +x "$PKG/CYD-Monitor.sh"
(
  cd "$STAGE"
  zip -qr "$OLDPWD/release/CYD-Monitor-portable.zip" CYD-Monitor
)
rm -rf "$STAGE"

echo "Built:"
ls -lah release/

# Refresh top-level downloadable packages when firmware bins exist.
if [[ -f ../firmware/release/esp32-cyd-pc-monitor-merged.bin ]]; then
  bash ../scripts/package_downloads.sh
fi
