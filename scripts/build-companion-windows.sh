#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
rustup target add x86_64-pc-windows-gnu >/dev/null
cargo build --no-default-features --features companion --bin cyd-companion \
  --release --target x86_64-pc-windows-gnu
mkdir -p dist/cyd-companion-windows
cp -f target/x86_64-pc-windows-gnu/release/cyd-companion.exe dist/cyd-companion-windows/
cp -f COMPANION.md dist/cyd-companion-windows/
cp -f packaging/README-windows.txt dist/cyd-companion-windows/README.txt

rm -f dist/cyd-companion-windows.zip dist/CYD-Companion-Portable.zip
( cd dist && zip -r cyd-companion-windows.zip cyd-companion-windows )
cp -f dist/cyd-companion-windows.zip dist/CYD-Companion-Portable.zip

# Windows setup wizard (NSIS) — primary user-friendly download
SETUP_EXE="dist/CYD-Companion-Setup.exe"
PORTABLE_ZIP="dist/CYD-Companion-Portable.zip"
if command -v makensis >/dev/null 2>&1; then
  makensis -V2 packaging/cyd-companion.nsi
  test -f "$SETUP_EXE"
  echo "Windows setup wizard ready: $SETUP_EXE"
else
  echo "warning: makensis not found — skipping CYD-Companion-Setup.exe" >&2
fi

mkdir -p /opt/cursor/artifacts
cp -f dist/cyd-companion-windows.zip /opt/cursor/artifacts/ 2>/dev/null || true
cp -f "$PORTABLE_ZIP" /opt/cursor/artifacts/ 2>/dev/null || true
if [[ -f "$SETUP_EXE" ]]; then
  cp -f "$SETUP_EXE" /opt/cursor/artifacts/
fi
ls -la dist/cyd-companion-windows.zip "$PORTABLE_ZIP" dist/cyd-companion-windows/ || true
[[ -f "$SETUP_EXE" ]] && ls -la "$SETUP_EXE"
echo "Windows companion ready: $PORTABLE_ZIP (+ zip alias + Setup.exe)"
