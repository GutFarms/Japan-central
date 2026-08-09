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
cp -f dist/cyd-companion-windows/README.txt dist/cyd-companion-windows/ 2>/dev/null || true
rm -f dist/cyd-companion-windows.zip
( cd dist && zip -r cyd-companion-windows.zip cyd-companion-windows )
cp -f dist/cyd-companion-windows.zip /opt/cursor/artifacts/ 2>/dev/null || true
ls -la dist/cyd-companion-windows.zip dist/cyd-companion-windows/
echo "Windows companion ready: dist/cyd-companion-windows.zip"
