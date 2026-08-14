#!/usr/bin/env bash
# Build Windows Companion + all-in-one CYD Miner Setup wizard.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT/companion"

VER="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')"

echo "==> Building egui Njörðr Seas' CYD miner (MinGW) v${VER}..."
rustup target add x86_64-pc-windows-gnu >/dev/null
cargo build --release --target x86_64-pc-windows-gnu

MERGED="$ROOT/flash/esp32-2432s028-sha256-miner-merged.bin"
APP_BIN="$ROOT/flash/esp32-2432s028-sha256-miner.bin"
SUMS="$ROOT/flash/SHA256SUMS.txt"
EXE_BUILT="$ROOT/companion/target/x86_64-pc-windows-gnu/release/cyd-companion.exe"
if [[ ! -f "$MERGED" ]]; then
  echo "error: missing $MERGED — run ./scripts/build-flash-images.sh first" >&2
  exit 1
fi
if [[ ! -f "$APP_BIN" ]]; then
  echo "error: missing $APP_BIN — Wi‑Fi OTA needs the app-only image" >&2
  exit 1
fi
test -f "$EXE_BUILT"

# Optional Authenticode (SmartScreen "Verified publisher"). Requires a real .pfx:
#   WINDOWS_CODE_SIGN_PFX=/path/to/cert.pfx
#   WINDOWS_CODE_SIGN_PASSWORD=...
# and osslsigncode on PATH. Unsigned builds still ship with PE VERSIONINFO + SHA256SUMS.
sign_win_pe() {
  local pe="$1"
  [[ -f "$pe" ]] || return 0
  if [[ -z "${WINDOWS_CODE_SIGN_PFX:-}" ]]; then
    return 0
  fi
  if ! command -v osslsigncode >/dev/null 2>&1; then
    echo "warning: WINDOWS_CODE_SIGN_PFX set but osslsigncode missing — skip signing $pe" >&2
    return 0
  fi
  local signed="${pe}.signed"
  echo "==> Authenticode signing $(basename "$pe")…"
  local -a cmd=(osslsigncode sign -pkcs12 "$WINDOWS_CODE_SIGN_PFX")
  if [[ -n "${WINDOWS_CODE_SIGN_PASSWORD:-}" ]]; then
    cmd+=(-pass "$WINDOWS_CODE_SIGN_PASSWORD")
  fi
  cmd+=(
    -n "Njörðr Seas' CYD miner"
    -i "https://github.com/GutFarms/Japan-central"
    -t "http://timestamp.digicert.com"
    -in "$pe" -out "$signed"
  )
  if "${cmd[@]}"; then
    mv -f "$signed" "$pe"
  else
    echo "warning: Authenticode sign failed for $pe — shipping unsigned" >&2
    rm -f "$signed"
  fi
}
sign_win_pe "$EXE_BUILT"

# Vendored Windows espflash for in-app Update board (no Python required).
ESPFLASH_VER="4.5.0"
ESPFLASH_EXE="$ROOT/packaging/Tools/espflash.exe"
if [[ ! -f "$ESPFLASH_EXE" ]]; then
  echo "==> Downloading espflash ${ESPFLASH_VER} (Windows)…"
  mkdir -p "$ROOT/packaging/Tools" /tmp/espflash-fetch
  curl -fsSL -o /tmp/espflash-fetch/espflash.zip \
    "https://github.com/esp-rs/espflash/releases/download/v${ESPFLASH_VER}/espflash-x86_64-pc-windows-msvc.zip"
  unzip -o /tmp/espflash-fetch/espflash.zip -d /tmp/espflash-fetch
  cp -f /tmp/espflash-fetch/espflash.exe "$ESPFLASH_EXE"
fi
test -f "$ESPFLASH_EXE"

KIT="$ROOT/dist/cyd-miner-kit"
rm -rf "$KIT"
mkdir -p "$KIT/Firmware" "$KIT/Tools"
cp -f "$EXE_BUILT" "$KIT/"
cp -f "$ROOT/packaging/icons/cyd-miner.ico" "$KIT/cyd-miner.ico"
cp -f "$ROOT/packaging/START-HERE.txt" "$KIT/"
cp -f "$ROOT/packaging/FLASH-WINDOWS.txt" "$KIT/"
cp -f "$ROOT/packaging/Flash-Firmware.bat" "$KIT/"
cp -f "$ROOT/COMPANION.md" "$KIT/"
cp -f "$ROOT/packaging/README-windows.txt" "$KIT/README.txt"
cp -f "$MERGED" "$KIT/Firmware/"
cp -f "$APP_BIN" "$KIT/Firmware/"
cp -f "$SUMS" "$KIT/Firmware/"
cp -f "$ROOT/FLASH.md" "$KIT/Firmware/"
echo "${VER}-sha256" > "$KIT/Firmware/VERSION.txt"
cp -f "$ESPFLASH_EXE" "$KIT/Tools/"
{
  echo "CYD Miner Kit ${VER}"
  echo "Companion: ${VER}"
  echo "Firmware: ${VER}-sha256"
  echo "USB flash: esp32-2432s028-sha256-miner-merged.bin @ 0x0"
  echo "Wi-Fi OTA: esp32-2432s028-sha256-miner.bin (app-only)"
  echo "In-app Update: Push (USB) / Push (Wi-Fi) / Flash (BOOT)"
  date -u +"Built: %Y-%m-%dT%H:%MZ"
} > "$KIT/VERSION.txt"

# Portable companion also gets Firmware + Tools so Update board works.
mkdir -p "$ROOT/dist/cyd-companion-windows/Firmware" "$ROOT/dist/cyd-companion-windows/Tools"
cp -f "$KIT/cyd-companion.exe" "$ROOT/dist/cyd-companion-windows/"
cp -f "$KIT/cyd-miner.ico" "$ROOT/dist/cyd-companion-windows/" 2>/dev/null || true
cp -f "$KIT/COMPANION.md" "$ROOT/dist/cyd-companion-windows/"
cp -f "$KIT/README.txt" "$ROOT/dist/cyd-companion-windows/README.txt"
cp -f "$KIT/START-HERE.txt" "$ROOT/dist/cyd-companion-windows/"
cp -f "$KIT/FLASH-WINDOWS.txt" "$ROOT/dist/cyd-companion-windows/"
cp -f "$MERGED" "$ROOT/dist/cyd-companion-windows/Firmware/"
cp -f "$APP_BIN" "$ROOT/dist/cyd-companion-windows/Firmware/"
cp -f "$SUMS" "$ROOT/dist/cyd-companion-windows/Firmware/"
echo "${VER}-sha256" > "$ROOT/dist/cyd-companion-windows/Firmware/VERSION.txt"
cp -f "$ESPFLASH_EXE" "$ROOT/dist/cyd-companion-windows/Tools/"

cd "$ROOT"
rm -f dist/cyd-companion-windows.zip dist/CYD-Companion-Portable.zip dist/CYD-Companion-App-Only.zip
rm -f dist/CYD-Miner-Portable.zip dist/CYD-Miner-Setup.exe dist/CYD-Companion-Setup.exe

( cd dist && zip -r cyd-companion-windows.zip cyd-companion-windows )
cp -f dist/cyd-companion-windows.zip dist/CYD-Companion-Portable.zip

# App kit: exe + Firmware + Tools so Update board works without the full installer.
rm -rf dist/cyd-companion-app-only
mkdir -p dist/cyd-companion-app-only/Firmware dist/cyd-companion-app-only/Tools
cp -f dist/cyd-companion-windows/cyd-companion.exe dist/cyd-companion-app-only/
cp -f dist/cyd-companion-windows/cyd-miner.ico dist/cyd-companion-app-only/ 2>/dev/null || \
  cp -f "$ROOT/packaging/icons/cyd-miner.ico" dist/cyd-companion-app-only/cyd-miner.ico
cp -f "$MERGED" dist/cyd-companion-app-only/Firmware/
cp -f "$APP_BIN" dist/cyd-companion-app-only/Firmware/
cp -f "$SUMS" dist/cyd-companion-app-only/Firmware/
echo "${VER}-sha256" > dist/cyd-companion-app-only/Firmware/VERSION.txt
cp -f "$ESPFLASH_EXE" dist/cyd-companion-app-only/Tools/
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
  sign_win_pe "$SETUP_EXE"
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
# Standalone app exe for direct download (no unzip).
cp -f dist/cyd-companion-windows/cyd-companion.exe flash/downloads/cyd-companion.exe
cp -f dist/cyd-companion-windows/cyd-companion.exe /opt/cursor/artifacts/ 2>/dev/null || true
# Tracked flasher so Update board can auto-download espflash.exe from the repo.
cp -f "$ESPFLASH_EXE" flash/downloads/espflash.exe
cp -f "$ESPFLASH_EXE" /opt/cursor/artifacts/ 2>/dev/null || true
# Keep raw firmware fetch URLs in sync (Fetch latest FW / ensure_firmware).
cp -f "$MERGED" flash/downloads/esp32-2432s028-sha256-miner-merged.bin
cp -f "$ROOT/flash/esp32-2432s028-sha256-miner.bin" flash/downloads/ 2>/dev/null || true
cp -f "$ROOT/flash/esp32-2432s028-sha256-miner-d0-merged.bin" flash/downloads/ 2>/dev/null || true
cp -f "$ROOT/flash/esp32-2432s028-sha256-miner-d0.bin" flash/downloads/ 2>/dev/null || true
echo "${VER}-sha256" > flash/downloads/VERSION.txt
echo "${VER}-sha256" > "$ROOT/flash/VERSION.txt"
if [[ -f "$SETUP_EXE" ]]; then
  cp -f "$SETUP_EXE" dist/CYD-Companion-Setup.exe /opt/cursor/artifacts/
  cp -f "$SETUP_EXE" dist/CYD-Companion-Setup.exe flash/downloads/ 2>/dev/null || true
fi

# Full downloadable SHA256SUMS (firmware + Windows packages) for SmartScreen / manual verify.
{
  echo "# Njörðr Seas' CYD miner ${VER} — verify with: sha256sum -c SHA256SUMS.txt"
  echo "# Firmware images"
  ( cd flash/downloads && sha256sum \
      esp32-2432s028-sha256-miner.bin \
      esp32-2432s028-sha256-miner-merged.bin \
      esp32-2432s028-sha256-miner-d0.bin \
      esp32-2432s028-sha256-miner-d0-merged.bin \
      2>/dev/null || true )
  echo "# Windows Companion packages"
  ( cd flash/downloads && sha256sum \
      cyd-companion.exe \
      CYD-Companion-App-Only.zip \
      CYD-Companion-Portable.zip \
      CYD-Miner-Portable.zip \
      CYD-Miner-Setup.exe \
      CYD-Companion-Setup.exe \
      espflash.exe \
      2>/dev/null || true )
} > flash/downloads/SHA256SUMS.txt
# Firmware-only sums stay next to flash/ images for flash scripts.
cp -f "$SUMS" flash/downloads/Firmware-SHA256SUMS.txt 2>/dev/null || true
cp -f flash/downloads/SHA256SUMS.txt /opt/cursor/artifacts/DOWNLOAD_SHA256SUMS.txt 2>/dev/null || true

# Keep a short download index next to the binaries.
cat > flash/downloads/README.md <<EOF
# Njörðr Seas' CYD miner downloads (\`${VER}\`)

Primary firmware is the **ESP32-D0** auto-tune build (\`esp32-2432s028-sha256-miner-d0-merged.bin\`, also the canonical \`esp32-2432s028-sha256-miner-merged.bin\`). Each board auto-benches on connect; **Bench boards (D0)** re-runs the path pick anytime.

| File | What it is |
|------|------------|
| **[CYD-Miner-Setup.exe](./CYD-Miner-Setup.exe)** | Installer — Companion + firmware (recommended) |
| **[cyd-companion.exe](./cyd-companion.exe)** | Standalone app — double-click to run |
| [CYD-Companion-App-Only.zip](./CYD-Companion-App-Only.zip) | App zip |
| [CYD-Miner-Portable.zip](./CYD-Miner-Portable.zip) | Full portable kit |
| [esp32-2432s028-sha256-miner-d0-merged.bin](./esp32-2432s028-sha256-miner-d0-merged.bin) | D0 firmware (preferred) |
| **[SHA256SUMS.txt](./SHA256SUMS.txt)** | SHA-256 of every downloadable file (verify before run) |

Firmware: [merged.bin](./esp32-2432s028-sha256-miner-merged.bin) @ \`0x0\` · [app.bin](./esp32-2432s028-sha256-miner.bin) (Wi‑Fi OTA) · [VERSION.txt](./VERSION.txt)

### Windows security / verified files
- PE metadata embeds publisher **GutFarms** + product/version (Explorer Details).
- Compare \`Get-FileHash <file> -Algorithm SHA256\` to [SHA256SUMS.txt](./SHA256SUMS.txt).
- If SmartScreen blocks: Properties → **Unblock**, then re-check the hash.
- Optional Authenticode: set \`WINDOWS_CODE_SIGN_PFX\` (+ password) when building for a Verified publisher signature.
EOF

ls -la dist/cyd-companion-windows/cyd-companion.exe "$PORTABLE_ZIP" "$APP_ONLY_ZIP" "$KIT_ZIP" || true
[[ -f "$SETUP_EXE" ]] && ls -la "$SETUP_EXE" dist/CYD-Companion-Setup.exe
echo "Windows kit ready (Setup wizard · Portable kit · App-only · downloadable .exe · SHA256SUMS)"
