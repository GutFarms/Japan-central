#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT/companion"

echo "==> Building egui CYD Companion (MinGW)..."
rustup target add x86_64-pc-windows-gnu >/dev/null
cargo build --release --target x86_64-pc-windows-gnu

mkdir -p "$ROOT/dist/cyd-companion-windows"
cp -f target/x86_64-pc-windows-gnu/release/cyd-companion.exe "$ROOT/dist/cyd-companion-windows/"
cp -f "$ROOT/COMPANION.md" "$ROOT/dist/cyd-companion-windows/"
cp -f "$ROOT/packaging/README-windows.txt" "$ROOT/dist/cyd-companion-windows/README.txt"

cd "$ROOT"
rm -f dist/cyd-companion-windows.zip dist/CYD-Companion-Portable.zip dist/CYD-Companion-App-Only.zip
( cd dist && zip -r cyd-companion-windows.zip cyd-companion-windows )
cp -f dist/cyd-companion-windows.zip dist/CYD-Companion-Portable.zip

mkdir -p dist/cyd-companion-app-only
cp -f dist/cyd-companion-windows/cyd-companion.exe dist/cyd-companion-app-only/
( cd dist && zip -r CYD-Companion-App-Only.zip cyd-companion-app-only )

SETUP_EXE="dist/CYD-Companion-Setup.exe"
PORTABLE_ZIP="dist/CYD-Companion-Portable.zip"
APP_ONLY_ZIP="dist/CYD-Companion-App-Only.zip"
if command -v makensis >/dev/null 2>&1; then
  makensis -V2 packaging/cyd-companion.nsi
  test -f "$SETUP_EXE"
  echo "Windows setup wizard ready: $SETUP_EXE"
else
  echo "warning: makensis not found — skipping CYD-Companion-Setup.exe" >&2
fi

mkdir -p /opt/cursor/artifacts flash/downloads
cp -f dist/cyd-companion-windows.zip /opt/cursor/artifacts/ 2>/dev/null || true
cp -f "$PORTABLE_ZIP" /opt/cursor/artifacts/ 2>/dev/null || true
cp -f "$APP_ONLY_ZIP" /opt/cursor/artifacts/ 2>/dev/null || true
cp -f "$PORTABLE_ZIP" "$APP_ONLY_ZIP" flash/downloads/ 2>/dev/null || true
if [[ -f "$SETUP_EXE" ]]; then
  cp -f "$SETUP_EXE" /opt/cursor/artifacts/
  cp -f "$SETUP_EXE" flash/downloads/ 2>/dev/null || true
fi
ls -la dist/cyd-companion-windows/cyd-companion.exe "$PORTABLE_ZIP" "$APP_ONLY_ZIP" || true
[[ -f "$SETUP_EXE" ]] && ls -la "$SETUP_EXE"
echo "Windows companion ready (egui graphics + USB netdata bridge)"
