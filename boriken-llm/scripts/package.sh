#!/usr/bin/env bash
# Build downloadable BorikenLLM packages into dist/ and /opt/cursor/artifacts/
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(python3 - <<'PY'
import json
from pathlib import Path
meta = json.loads(Path("corpus/vocabulary.json").read_text())["meta"]
print(meta.get("version", "0.2.0"))
PY
)"
DIST="$ROOT/dist"
ART="/opt/cursor/artifacts"
mkdir -p "$DIST" "$ART"

echo "==> Building offline web learner"
python3 scripts/build_webapp.py

echo "==> Ensuring training corpus exists"
python3 train/build_dataset.py >/dev/null

# --- 1) Offline web learner (open on iPhone) ---
WEB_NAME="Boriken-Offline-Learner-${VERSION}"
WEB_DIR="$DIST/$WEB_NAME"
rm -rf "$WEB_DIR"
mkdir -p "$WEB_DIR"
cp -R webapp/. "$WEB_DIR/"
(
  cd "$DIST"
  zip -r -q "${WEB_NAME}.zip" "$WEB_NAME"
  tar -czf "${WEB_NAME}.tar.gz" "$WEB_NAME"
)

# --- 2) iOS content bundle (drop into Xcode / app resources) ---
IOS_NAME="Boriken-iOS-ContentBundle-${VERSION}"
IOS_DIR="$DIST/$IOS_NAME"
rm -rf "$IOS_DIR"
mkdir -p "$IOS_DIR/Corpus" "$IOS_DIR/Model" "$IOS_DIR/BorikenKit"
cp corpus/vocabulary.json corpus/grammar.json corpus/sentences.json corpus/history.json "$IOS_DIR/Corpus/"
cp -R corpus/training "$IOS_DIR/Corpus/" 2>/dev/null || true
cp models/boriken-gpt.pt models/boriken-gpt.json "$IOS_DIR/Model/" 2>/dev/null || true
cp -R ios/BorikenKit "$IOS_DIR/"
cp ios/SampleApp/BorikenHomeView.swift "$IOS_DIR/"
cat > "$IOS_DIR/README.txt" <<EOF
Boriken iOS Content Bundle ${VERSION}
====================================

1. In Xcode: File → Add Package Dependencies → Add Local → select BorikenKit/
2. Drag Corpus/ into your app target (Copy items if needed)
3. Optionally ship Model/boriken-gpt.pt for on-device completion research
4. Add BorikenHomeView.swift to a SwiftUI target, or call BorikenClient

Point BorikenClient at your API host, or use the Offline Learner zip for
zero-server practice on device (open index.html / Add to Home Screen).
EOF
(
  cd "$DIST"
  zip -r -q "${IOS_NAME}.zip" "$IOS_NAME"
  tar -czf "${IOS_NAME}.tar.gz" "$IOS_NAME"
)

# --- 3) High-graphics desktop app ---
DESK_NAME="Boriken-Desktop-${VERSION}-x86_64-linux"
DESK_DIR="$DIST/$DESK_NAME"
rm -rf "$DESK_DIR"
mkdir -p "$DESK_DIR/corpus"
echo "==> Building high-graphics desktop release"
(
  cd desktop
  cargo build --release
)
cp desktop/target/release/boriken-desktop "$DESK_DIR/"
cp corpus/vocabulary.json corpus/grammar.json corpus/sentences.json corpus/history.json "$DESK_DIR/corpus/"
cp prompts/system.md "$DESK_DIR/" 2>/dev/null || true
# keep ACCURACY.md with desktop when present
cp ACCURACY.md "$DESK_DIR/" 2>/dev/null || true
cat > "$DESK_DIR/RUN.txt" <<EOF
BORIKÉN Desktop Learner ${VERSION}
=================================

High-graphics offline classroom (cinematic sun, ocean parallax,
particle weather, XP games).

Linux:
  ./boriken-desktop

Ubuntu/Debian once (if the window fails to open):
  sudo apt install libxkbcommon-x11-0 libxcb-xkb1

Modes: Word of Day · Batey Match · Memory Flip · Konuko Fill ·
Areyto Quest · Define

Corpus ships beside the binary (corpus/vocabulary.json).
EOF
cp desktop/Cargo.toml "$DESK_DIR/BUILD.txt" 2>/dev/null || true
(
  cd "$DIST"
  tar -czf "${DESK_NAME}.tar.gz" "$DESK_NAME"
  zip -r -q "${DESK_NAME}.zip" "$DESK_NAME"
)

# --- 4) Full toolkit (API + model + corpus + iOS + web + desktop sources) ---
TOOL_NAME="BorikenLLM-Toolkit-${VERSION}"
TOOL_DIR="$DIST/$TOOL_NAME"
rm -rf "$TOOL_DIR"
mkdir -p "$TOOL_DIR"
for item in api corpus engine ios models prompts train tests webapp desktop \
            requirements.txt README.md DOWNLOAD.md Makefile scripts; do
  cp -R "$item" "$TOOL_DIR/" 2>/dev/null || true
done
# drop heavy target/ from toolkit if copied
rm -rf "$TOOL_DIR/desktop/target"
# lightweight start helpers
cat > "$TOOL_DIR/START.txt" <<EOF
BorikenLLM Toolkit ${VERSION}
=============================

Option A — Offline learner (no install)
  open webapp/index.html
  (on iPhone: copy zip → Files → open index.html → Share → Add to Home Screen)

Option B — High-graphics desktop
  cd desktop && cargo run --release
  # or use the prebuilt: dist/Boriken-Desktop-*-linux.tar.gz

Option C — Local API for the iOS app
  python3 -m pip install -r requirements.txt
  python3 -m uvicorn api.server:app --host 0.0.0.0 --port 8080
  # then point BorikenKit at http://<your-lan-ip>:8080

Option D — Rebuild model
  make train

Fun endpoints: http://127.0.0.1:8080/v1/fun/menu
Definitions:   http://127.0.0.1:8080/v1/define/huracan
EOF
chmod +x "$TOOL_DIR/scripts/"*.sh "$TOOL_DIR/scripts/"*.py 2>/dev/null || true
(
  cd "$DIST"
  zip -r -q "${TOOL_NAME}.zip" "$TOOL_NAME"
  tar -czf "${TOOL_NAME}.tar.gz" "$TOOL_NAME"
)

# Copy primary downloadables to Cursor artifacts for PR / agent downloads
cp "$DIST/${WEB_NAME}.zip" "$ART/"
cp "$DIST/${IOS_NAME}.zip" "$ART/"
cp "$DIST/${TOOL_NAME}.zip" "$ART/"
cp "$DIST/${DESK_NAME}.tar.gz" "$ART/"
cp "$DIST/${DESK_NAME}.zip" "$ART/"
cp "$DIST/${WEB_NAME}.tar.gz" "$ART/" 2>/dev/null || true

# Stable short names
cp "$DIST/${WEB_NAME}.zip" "$ART/Boriken-Offline-Learner.zip"
cp "$DIST/${IOS_NAME}.zip" "$ART/Boriken-iOS-ContentBundle.zip"
cp "$DIST/${TOOL_NAME}.zip" "$ART/BorikenLLM-Toolkit.zip"
cp "$DIST/${DESK_NAME}.tar.gz" "$ART/Boriken-Desktop-linux.tar.gz"
cp "$DIST/${DESK_NAME}.zip" "$ART/Boriken-Desktop-linux.zip"
cp "$DIST/${WEB_NAME}.zip" "$DIST/Boriken-Offline-Learner.zip"
cp "$DIST/${IOS_NAME}.zip" "$DIST/Boriken-iOS-ContentBundle.zip"
cp "$DIST/${TOOL_NAME}.zip" "$DIST/BorikenLLM-Toolkit.zip"
cp "$DIST/${DESK_NAME}.tar.gz" "$DIST/Boriken-Desktop-linux.tar.gz"
cp "$DIST/${DESK_NAME}.zip" "$DIST/Boriken-Desktop-linux.zip"

echo "==> Downloadables ready"
ls -lh "$DIST"/Boriken*.{zip,tar.gz} "$ART"/Boriken*.{zip,tar.gz} 2>/dev/null | sed 's|/workspace/boriken-llm/||'
