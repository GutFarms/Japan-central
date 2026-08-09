#!/usr/bin/env bash
# Assemble a portable Windows zip from built .exe files.
# Usage:
#   GUI_EXE=... LOCAL_EXE=... ./scripts/package-windows.sh
# Or from CI after cargo build --release:
#   ./scripts/package-windows.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="$(tr -d '[:space:]' <"$ROOT/VERSION")"
OUT_DIR="${OUT_DIR:-$ROOT/dist}"
STAGE="$OUT_DIR/cursor-appliance-windows-x64"
ZIP="$OUT_DIR/cursor-appliance-windows-x64-v${VERSION}.zip"

GUI_EXE="${GUI_EXE:-}"
LOCAL_EXE="${LOCAL_EXE:-}"

if [[ -z "$GUI_EXE" ]]; then
  for c in \
    "$ROOT/gui/target/x86_64-pc-windows-msvc/release/cursor-appliance-gui.exe" \
    "$ROOT/gui/target/x86_64-pc-windows-gnu/release/cursor-appliance-gui.exe" \
    "$ROOT/gui/target/release/cursor-appliance-gui.exe"; do
    [[ -f "$c" ]] && GUI_EXE="$c" && break
  done
fi
if [[ -z "$LOCAL_EXE" ]]; then
  for c in \
    "$ROOT/gui/target/x86_64-pc-windows-msvc/release/cursor-local-worker.exe" \
    "$ROOT/gui/target/x86_64-pc-windows-gnu/release/cursor-local-worker.exe" \
    "$ROOT/gui/target/release/cursor-local-worker.exe"; do
    [[ -f "$c" ]] && LOCAL_EXE="$c" && break
  done
fi

if [[ -z "$GUI_EXE" || -z "$LOCAL_EXE" ]]; then
  echo "error: Windows .exe builds not found." >&2
  echo "Build on Windows with: cargo build --release --bins" >&2
  echo "Or set GUI_EXE= and LOCAL_EXE=." >&2
  exit 1
fi

rm -rf "$STAGE"
mkdir -p "$STAGE/scripts/windows" "$STAGE/data/local-queue/incoming" \
  "$STAGE/data/local-queue/done" "$STAGE/data/local-queue/failed" \
  "$STAGE/sandbox" "$STAGE/sandbox-workspace" "$STAGE/sandbox-home" "$STAGE/sandbox-tmp"

cp "$GUI_EXE" "$STAGE/CursorAppliance.exe"
cp "$LOCAL_EXE" "$STAGE/cursor-local-worker.exe"
cp "$ROOT/Start-CursorAppliance.bat" "$STAGE/"
cp "$ROOT/Start-PortableSandbox.bat" "$STAGE/"
cp "$ROOT/.env.example" "$STAGE/"
cp "$ROOT/VERSION" "$STAGE/"
cp "$ROOT/WINDOWS.md" "$STAGE/"
cp "$ROOT/README.md" "$STAGE/"
cp "$ROOT/sandbox/README.txt" "$STAGE/sandbox/"
cp "$ROOT/scripts/windows/"*.ps1 "$STAGE/scripts/windows/"
cp "$ROOT/scripts/windows/"*.cmd "$STAGE/scripts/windows/" 2>/dev/null || true

# Sensible Windows defaults in packaged .env.example notes stay; create starter .env
cp "$STAGE/.env.example" "$STAGE/.env"

# Keep worker_dir empty so GUI seeds to parent or appliance root on first run.
cat >"$STAGE/README-WINDOWS.txt" <<EOF
Cursor Appliance for Windows v${VERSION}
======================================

Quick start
-----------
1. Unzip this folder anywhere (e.g. Desktop\\cursor-appliance).
2. Double-click Start-CursorAppliance.bat
   OR for a portable sandbox: Start-PortableSandbox.bat
3. Optional cloud worker:
     powershell -ExecutionPolicy Bypass -File .\\scripts\\windows\\Setup.ps1
     powershell -ExecutionPolicy Bypass -File .\\scripts\\windows\\Start-CloudWorker.ps1

Portable sandbox keeps data/profile/temp inside this folder, and uses
Windows Sandbox when available (see WINDOWS.md).
EOF

mkdir -p "$OUT_DIR"
rm -f "$ZIP"
if command -v zip >/dev/null 2>&1; then
  (cd "$OUT_DIR" && zip -r "$(basename "$ZIP")" "$(basename "$STAGE")")
elif command -v 7z >/dev/null 2>&1; then
  7z a -tzip "$ZIP" "$STAGE"
else
  # Fallback: tar.gz still useful; CI on Windows uses Compress-Archive
  tar -C "$OUT_DIR" -czf "${ZIP%.zip}.tar.gz" "$(basename "$STAGE")"
  echo "warn: zip not found; wrote ${ZIP%.zip}.tar.gz"
  echo "$OUT_DIR/$(basename "$STAGE")"
  exit 0
fi

echo "Packed: $ZIP"
ls -lh "$ZIP"
