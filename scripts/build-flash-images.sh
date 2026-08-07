#!/usr/bin/env bash
# Build release firmware and write flashable .bin images into ./flash/
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

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

mkdir -p flash
echo "==> Building xtensa-esp32-none-elf release (esp,lite)..."
cargo +esp build -Zbuild-std=core,alloc --release \
  --target xtensa-esp32-none-elf --features esp,lite

ELF="$ROOT/target/xtensa-esp32-none-elf/release/esp32-s3-scrypt-miner"
test -f "$ELF"

echo "==> Writing app image..."
espflash save-image --chip esp32 --flash-size 4mb \
  "$ELF" flash/esp32-2432s028-scrypt-miner.bin

echo "==> Writing merged image (flash at 0x0)..."
espflash save-image --chip esp32 --flash-size 4mb --merge \
  "$ELF" flash/esp32-2432s028-scrypt-miner-merged.bin

ls -la flash/esp32-2432s028-scrypt-miner*.bin
echo "Done. See FLASH.md for espflash write-bin / flash commands."
