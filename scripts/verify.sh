#!/usr/bin/env bash
# Rebuild firmware, refresh release bins, and smoke-test the host agent.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PIO="${PIO:-}"
if [[ -z "$PIO" ]]; then
  if command -v pio >/dev/null 2>&1; then
    PIO="$(command -v pio)"
  elif [[ -x /tmp/pio-venv/bin/pio ]]; then
    PIO=/tmp/pio-venv/bin/pio
  else
    echo "PlatformIO CLI (pio) not found" >&2
    exit 1
  fi
fi

ESPTOOL_PY="${ESPTOOL_PY:-}"
if [[ -z "$ESPTOOL_PY" ]]; then
  if [[ -f "$HOME/.platformio/packages/tool-esptoolpy/esptool.py" ]]; then
    ESPTOOL_PY="$HOME/.platformio/packages/tool-esptoolpy/esptool.py"
  fi
fi

echo "==> Building firmware"
cd "$ROOT/firmware"
"$PIO" run

echo "==> Exporting release binaries"
mkdir -p release
cp .pio/build/esp32-cyd/firmware.bin release/esp32-cyd-pc-monitor.bin
cp .pio/build/esp32-cyd/bootloader.bin release/bootloader.bin
cp .pio/build/esp32-cyd/partitions.bin release/partitions.bin

PYTHON=python3
if [[ -x /tmp/pio-venv/bin/python ]]; then
  PYTHON=/tmp/pio-venv/bin/python
fi

if [[ -n "${ESPTOOL_PY}" && -f "$ESPTOOL_PY" ]]; then
  "$PYTHON" "$ESPTOOL_PY" --chip esp32 merge_bin \
    -o release/esp32-cyd-pc-monitor-merged.bin \
    --flash_mode dio --flash_freq 40m --flash_size 4MB \
    0x1000 .pio/build/esp32-cyd/bootloader.bin \
    0x8000 .pio/build/esp32-cyd/partitions.bin \
    0x10000 .pio/build/esp32-cyd/firmware.bin
else
  echo "esptool.py not found; skipped merged image" >&2
fi

echo "==> Checking credential strings in app binary"
strings release/esp32-cyd-pc-monitor.bin | grep -F "Stargate Command" >/dev/null
strings release/esp32-cyd-pc-monitor.bin | grep -F "ESP32-CYD PC/GPU Monitor" >/dev/null

echo "==> Host agent smoke tests"
HOST_PY="$ROOT/host/.venv/bin/python"
if [[ ! -x "$HOST_PY" ]]; then
  python3 -m venv "$ROOT/host/.venv"
  HOST_PY="$ROOT/host/.venv/bin/python"
  "$HOST_PY" -m pip install -q -r "$ROOT/host/requirements.txt"
fi

"$HOST_PY" "$ROOT/host/agent.py" --help >/dev/null
"$HOST_PY" "$ROOT/scripts/smoke_host.py"

echo "OK: firmware + host verified"
