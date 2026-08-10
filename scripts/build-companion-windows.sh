#!/usr/bin/env bash
# Build Windows Companion + all-in-one CYD Miner Setup wizard.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT/companion"

VER="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')"

echo "==> Building egui CYD Companion (MinGW) v${VER}..."
rustup target add x86_64-pc-windows-gnu >/dev/null
cargo build --release --target x86_64-pc-windows-gnu

MERGED="$ROOT/flash/esp32-2432s028-sha256-miner-merged.bin"
SUMS="$ROOT/flash/SHA256SUMS.txt"
if [[ ! -f "$MERGED" ]]; then
  echo "error: missing $MERGED — run ./scripts/build-flash-images.sh first" >&2
  exit 1
fi

KIT="$ROOT/dist/cyd-miner-kit"
rm -rf "$KIT"
mkdir -p "$KIT/Firmware"
cp -f target/x86_64-pc-windows-gnu/release/cyd-companion.exe "$KIT/"
cp -f "$ROOT/packaging/START-HERE.txt" "$KIT/"
cp -f "$ROOT/packaging/FLASH-WINDOWS.txt" "$KIT/"
cp -f "$ROOT/packaging/Flash-Firmware.bat" "$KIT/"
cp -f "$ROOT/COMPANION.md" "$KIT/"
cp -f "$ROOT/packaging/README-windows.txt" "$KIT/README.txt"
cp -f "$MERGED" "$KIT/Firmware/"
cp -f "$SUMS" "$KIT/Firmware/"
cp -f "$ROOT/FLASH.md" "$KIT/Firmware/"
{
  echo "CYD Miner Kit ${VER}"
  echo "Companion: ${VER}"
  echo "Firmware image: esp32-2432s028-sha256-miner-merged.bin"
  echo "Flash offset: 0x0 (DIO, 4MB, 40MHz)"
  date -u +"Built: %Y-%m-%dT%H:%MZ"
} > "$KIT/VERSION.txt"

# Legacy portable layout (app docs only) + app-only zip
mkdir -p "$ROOT/dist/cyd-companion-windows"
cp -f "$KIT/cyd-companion.exe" "$ROOT/dist/cyd-companion-windows/"
cp -f "$KIT/COMPANION.md" "$ROOT/dist/cyd-companion-windows/"
cp -f "$KIT/README.txt" "$ROOT/dist/cyd-companion-windows/README.txt"
cp -f "$KIT/START-HERE.txt" "$ROOT/dist/cyd-companion-windows/"
cp -f "$KIT/FLASH-WINDOWS.txt" "$ROOT/dist/cyd-companion-windows/"

cd "$ROOT"
rm -f dist/cyd-companion-windows.zip dist/CYD-Companion-Portable.zip dist/CYD-Companion-App-Only.zip
rm -f dist/CYD-Miner-Portable.zip dist/CYD-Miner-Setup.exe dist/CYD-Companion-Setup.exe

( cd dist && zip -r cyd-companion-windows.zip cyd-companion-windows )
cp -f dist/cyd-companion-windows.zip dist/CYD-Companion-Portable.zip

mkdir -p dist/cyd-companion-app-only
cp -f dist/cyd-companion-windows/cyd-companion.exe dist/cyd-companion-app-only/
( cd dist && zip -r CYD-Companion-App-Only.zip cyd-companion-app-only )

# Full kit portable zip (app + firmware + flash helper)
( cd dist && zip -r CYD-Miner-Portable.zip cyd-miner-kit )

SETUP_EXE="dist/CYD-Miner-Setup.exe"
PORTABLE_ZIP="dist/CYD-Companion-Portable.zip"
APP_ONLY_ZIP="dist/CYD-Companion-App-Only.zip"
KIT_ZIP="dist/CYD-Miner-Portable.zip"

if command -v makensis >/dev/null 2>&1; then
  # Compile from packaging/ so relative File/License paths resolve.
  sed "s/!define PRODUCT_VERSION \".*\"/!define PRODUCT_VERSION \"${VER}\"/" \
    packaging/cyd-miner.nsi > packaging/cyd-miner-build.nsi
  ( cd packaging && makensis -V2 cyd-miner-build.nsi )
  rm -f packaging/cyd-miner-build.nsi
  test -f "$SETUP_EXE"
  # Keep old filename as alias for existing links/docs
  cp -f "$SETUP_EXE" dist/CYD-Companion-Setup.exe
  echo "Windows setup wizard ready: $SETUP_EXE (+ CYD-Companion-Setup.exe alias)"
else
  echo "warning: makensis not found — skipping CYD-Miner-Setup.exe" >&2
fi

mkdir -p /opt/cursor/artifacts flash/downloads
cp -f dist/cyd-companion-windows.zip /opt/cursor/artifacts/ 2>/dev/null || true
cp -f "$PORTABLE_ZIP" /opt/cursor/artifacts/ 2>/dev/null || true
cp -f "$APP_ONLY_ZIP" /opt/cursor/artifacts/ 2>/dev/null || true
cp -f "$KIT_ZIP" /opt/cursor/artifacts/ 2>/dev/null || true
cp -f "$PORTABLE_ZIP" "$APP_ONLY_ZIP" flash/downloads/ 2>/dev/null || true
cp -f "$KIT_ZIP" flash/downloads/ 2>/dev/null || true
if [[ -f "$SETUP_EXE" ]]; then
  cp -f "$SETUP_EXE" dist/CYD-Companion-Setup.exe /opt/cursor/artifacts/
  cp -f "$SETUP_EXE" dist/CYD-Companion-Setup.exe flash/downloads/ 2>/dev/null || true
fi

ls -la dist/cyd-companion-windows/cyd-companion.exe "$PORTABLE_ZIP" "$APP_ONLY_ZIP" "$KIT_ZIP" || true
[[ -f "$SETUP_EXE" ]] && ls -la "$SETUP_EXE" dist/CYD-Companion-Setup.exe
echo "Windows kit ready (Setup wizard · Portable kit · App-only)"
