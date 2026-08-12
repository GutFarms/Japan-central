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
cp corpus/vocabulary.json corpus/grammar.json corpus/sentences.json "$IOS_DIR/Corpus/"
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

# --- 3) Full toolkit (API + model + corpus + iOS + web) ---
TOOL_NAME="BorikenLLM-Toolkit-${VERSION}"
TOOL_DIR="$DIST/$TOOL_NAME"
rm -rf "$TOOL_DIR"
mkdir -p "$TOOL_DIR"
for item in api corpus engine ios models prompts train tests webapp \
            requirements.txt README.md Makefile scripts; do
  cp -R "$item" "$TOOL_DIR/" 2>/dev/null || true
done
# lightweight start helpers
cat > "$TOOL_DIR/START.txt" <<EOF
BorikenLLM Toolkit ${VERSION}
=============================

Option A — Offline learner (no install)
  open webapp/index.html
  (on iPhone: copy zip → Files → open index.html → Share → Add to Home Screen)

Option B — Local API for the iOS app
  python3 -m pip install -r requirements.txt
  python3 -m uvicorn api.server:app --host 0.0.0.0 --port 8080
  # then point BorikenKit at http://<your-lan-ip>:8080

Option C — Rebuild model
  make train

Fun endpoints: http://127.0.0.1:8080/v1/fun/menu
Definitions:   http://127.0.0.1:8080/v1/define/huracan
EOF
cp START.txt "$TOOL_DIR/START.txt" 2>/dev/null || true
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
cp "$DIST/${WEB_NAME}.tar.gz" "$ART/" 2>/dev/null || true

# Stable short names
cp "$DIST/${WEB_NAME}.zip" "$ART/Boriken-Offline-Learner.zip"
cp "$DIST/${IOS_NAME}.zip" "$ART/Boriken-iOS-ContentBundle.zip"
cp "$DIST/${TOOL_NAME}.zip" "$ART/BorikenLLM-Toolkit.zip"
cp "$DIST/${WEB_NAME}.zip" "$DIST/Boriken-Offline-Learner.zip"
cp "$DIST/${IOS_NAME}.zip" "$DIST/Boriken-iOS-ContentBundle.zip"
cp "$DIST/${TOOL_NAME}.zip" "$DIST/BorikenLLM-Toolkit.zip"

echo "==> Downloadables ready"
ls -lh "$DIST"/*.zip "$ART"/Boriken*.zip
