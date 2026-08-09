#!/usr/bin/env bash
# Build release firmware and write flashable .bin images into ./flash/
# Default: companion-first C++ firmware (firmware-cpp / PlatformIO).
# Set BUILD_RUST=1 to use the legacy Rust/esp-hal path instead.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

APP_BIN="flash/esp32-2432s028-scrypt-miner.bin"
MERGED_BIN="flash/esp32-2432s028-scrypt-miner-merged.bin"
mkdir -p flash

if [[ "${BUILD_RUST:-0}" != "1" && -d "$ROOT/firmware-cpp" ]]; then
  export PATH="${HOME}/.local/bin:${PATH}"
  if ! command -v pio >/dev/null 2>&1; then
    echo "ERROR: PlatformIO (pio) not found — install or set BUILD_RUST=1" >&2
    exit 1
  fi
  echo "==> Building C++ CYD firmware (PlatformIO)..."
  pio run -d "$ROOT/firmware-cpp" -e cyd
else
  source_esp_env() {
    local f
    for f in "$ROOT/export-esp.sh" "$HOME/export-esp.sh" "$ROOT/export-esp.sh.example"; do
      if [[ -f "$f" ]]; then
        # shellcheck disable=SC1090
        source "$f"
        return 0
      fi
    done
    echo "warning: no export-esp.sh found; relying on PATH" >&2
  }
  source_esp_env

  echo "==> Building xtensa-esp32-none-elf release (esp,lite)..."
  cargo +esp build -Zbuild-std=core,alloc --release \
    --target xtensa-esp32-none-elf --features esp,lite

  ELF="$ROOT/target/xtensa-esp32-none-elf/release/esp32-s3-scrypt-miner"
  test -f "$ELF"

  FLASH_ARGS=(--chip esp32 --flash-size 4mb --flash-mode dio --flash-freq 40mhz)

  echo "==> Writing app image (flash @ 0x10000, DIO @ 40 MHz, 4 MB)..."
  espflash save-image "${FLASH_ARGS[@]}" \
    "$ELF" "$APP_BIN"

  echo "==> Writing merged image (flash @ 0x0, no 4MiB pad)..."
  espflash save-image "${FLASH_ARGS[@]}" --merge --skip-padding \
    "$ELF" "$MERGED_BIN"
fi

# Basic sanity: ESP app magic 0xE9 at start of app image and at 0x10000 in merged.
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
mode = app[2]
size_freq = app[3]
if mode != 0x02:
    print(f"ERROR: flash mode byte={mode:#04x}, expected DIO (0x02)", file=sys.stderr); ok = False
# Arduino/esptool may encode size/freq differently than espflash; accept common 4MB@40MHz forms.
freq = size_freq & 0x0F
size = size_freq >> 4
if freq not in (0x0, 0x1):  # 40MHz or 26MHz
    print(f"WARNING: flash freq nibble={freq:#x} (expected 40MHz=0x0)", file=sys.stderr)
if size not in (0x2, 0x3):  # 4MB or 8MB
    print(f"WARNING: flash size nibble={size:#x} (expected 4MB=0x2)", file=sys.stderr)
print(f"Header OK: magic=0xE9 mode=DIO size/freq={size_freq:#04x}")
sys.exit(0 if ok else 1)
PY

echo "==> Writing SHA256SUMS.txt..."
(
  cd flash
  sha256sum esp32-2432s028-scrypt-miner.bin esp32-2432s028-scrypt-miner-merged.bin > SHA256SUMS.txt
  cat SHA256SUMS.txt
)

ls -la "$APP_BIN" "$MERGED_BIN"
echo "Done. Save/flash via: ./scripts/serve-web-flasher.sh  →  http://127.0.0.1:8080/web/"
