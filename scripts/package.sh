#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
TARGET_TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
OUT_DIR="dist/grok-agent-${VERSION}-${TARGET_TRIPLE}"
ARCHIVE="dist/grok-agent-${VERSION}-${TARGET_TRIPLE}.tar.gz"

echo "==> Building release binary"
cargo build --release

echo "==> Assembling package at ${OUT_DIR}"
rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR"
cp target/release/grok-agent "$OUT_DIR/"
cp README.md "$OUT_DIR/"
cp .env.example "$OUT_DIR/"
cp packaging/grok-agent.desktop "$OUT_DIR/" 2>/dev/null || true

cat > "$OUT_DIR/RUN.txt" <<EOF
grok-agent ${VERSION}
====================

1. Get an API key from https://console.x.ai/
2. On Ubuntu/Debian, ensure GUI libs are installed:
     sudo apt install libxkbcommon-x11-0 libxcb-xkb1
     # if the app complains about libxkbcommon-x11.so:
     sudo ln -sf libxkbcommon-x11.so.0 /usr/lib/x86_64-linux-gnu/libxkbcommon-x11.so
3. Launch the app:
     ./grok-agent
4. Open Settings and paste your API key (saved to ~/.grok-agent/settings.json)

Optional CLI mode:
  ./grok-agent --cli
  ./grok-agent -p "Summarize this folder"

Linux desktop launcher:
  copy grok-agent.desktop to ~/.local/share/applications/
  and edit Exec= to the absolute path of this binary
EOF

mkdir -p dist
tar -C dist -czf "$ARCHIVE" "$(basename "$OUT_DIR")"

echo "==> Created ${ARCHIVE}"
ls -lh "$ARCHIVE" "$OUT_DIR/grok-agent"
