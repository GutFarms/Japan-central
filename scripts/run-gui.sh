#!/usr/bin/env bash
# Build (if needed) and launch the Cursor Appliance egui control panel.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GUI_DIR="$ROOT/gui"

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "error: required command not found: $1" >&2
    exit 1
  }
}

need_cmd cargo

cd "$GUI_DIR"
if [[ "${CURSOR_APPLIANCE_GUI_RELEASE:-0}" == "1" ]]; then
  cargo build --release --bins
  exec "$GUI_DIR/target/release/cursor-appliance-gui" "$@"
fi

cargo build --bins
exec "$GUI_DIR/target/debug/cursor-appliance-gui" "$@"
