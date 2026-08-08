#!/usr/bin/env bash
# Start the lightweight local worker (no Cursor cloud connection).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO_ROOT="$(cd "$ROOT/.." && pwd)"
GUI_DIR="$ROOT/gui"

load_env_file() {
  local file="$1"
  [[ -f "$file" ]] || return 0
  set -a
  # shellcheck disable=SC1090
  source "$file"
  set +a
}

load_env_file "$ROOT/.env"
load_env_file "$ROOT/config/appliance.env"

NAME="${CURSOR_APPLIANCE_NAME:-cursor-appliance}"
WORKER_DIR="${CURSOR_APPLIANCE_WORKER_DIR:-$REPO_ROOT}"
DATA_DIR="${CURSOR_APPLIANCE_DATA_DIR:-$ROOT/data}"
LOCAL_ADDR="${CURSOR_APPLIANCE_LOCAL_ADDR:-127.0.0.1:8734}"

mkdir -p "$DATA_DIR/local-queue/incoming" "$DATA_DIR/local-queue/done" "$DATA_DIR/local-queue/failed"

BIN=""
for candidate in \
  "$GUI_DIR/target/release/cursor-local-worker" \
  "$GUI_DIR/target/debug/cursor-local-worker"; do
  if [[ -x "$candidate" ]]; then
    BIN="$candidate"
    break
  fi
done

if [[ -z "$BIN" ]]; then
  if ! command -v cargo >/dev/null 2>&1; then
    echo "error: cursor-local-worker binary missing and cargo not found" >&2
    echo "Build with: (cd $GUI_DIR && cargo build --bin cursor-local-worker)" >&2
    exit 1
  fi
  echo "Building cursor-local-worker…"
  (cd "$GUI_DIR" && cargo build --bin cursor-local-worker)
  BIN="$GUI_DIR/target/debug/cursor-local-worker"
fi

echo "Starting local worker"
echo "  name:   $NAME"
echo "  repo:   $WORKER_DIR"
echo "  health: http://${LOCAL_ADDR}/healthz"
echo "  status: http://${LOCAL_ADDR}/status"

exec "$BIN" \
  --name "$NAME" \
  --worker-dir "$WORKER_DIR" \
  --data-dir "$DATA_DIR" \
  --listen "$LOCAL_ADDR"
