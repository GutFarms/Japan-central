#!/usr/bin/env bash
# Launch a long-lived Cursor My Machines worker for this appliance.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO_ROOT="$(cd "$ROOT/.." && pwd)"

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

# GUI / operator offline latch — refuse to dial Cursor cloud.
OFFLINE="$(echo "${CURSOR_APPLIANCE_OFFLINE:-0}" | tr '[:upper:]' '[:lower:]')"
if [[ "$OFFLINE" == "1" || "$OFFLINE" == "true" || "$OFFLINE" == "yes" || "$OFFLINE" == "on" ]]; then
  echo "error: CURSOR_APPLIANCE_OFFLINE=${CURSOR_APPLIANCE_OFFLINE} — cloud worker disabled" >&2
  echo "Disable Offline mode in the GUI (or set CURSOR_APPLIANCE_OFFLINE=0) to connect." >&2
  exit 2
fi

NAME="${CURSOR_APPLIANCE_NAME:-cursor-appliance}"
WORKER_DIR="${CURSOR_APPLIANCE_WORKER_DIR:-$REPO_ROOT}"
MGMT_ADDR="${CURSOR_APPLIANCE_MANAGEMENT_ADDR:-127.0.0.1:8733}"
DATA_DIR="${CURSOR_APPLIANCE_DATA_DIR:-$ROOT/data}"
AUTH_TOKEN_FILE="${CURSOR_APPLIANCE_AUTH_TOKEN_FILE:-}"
# 0 = stay connected forever (dedicated appliance). CLI default is 3600s.
IDLE_TIMEOUT="${CURSOR_APPLIANCE_IDLE_RELEASE_TIMEOUT:-0}"

mkdir -p "$DATA_DIR"

if ! command -v agent >/dev/null 2>&1; then
  echo "error: Cursor Agent CLI (agent) not found on PATH" >&2
  echo "Run: $ROOT/scripts/setup.sh" >&2
  exit 1
fi

if [[ ! -d "$WORKER_DIR/.git" ]]; then
  echo "error: worker dir is not a git checkout: $WORKER_DIR" >&2
  echo "My Machines registers the git remote from --worker-dir." >&2
  exit 1
fi

# Global agent auth (also accepts CURSOR_API_KEY from the environment).
agent_globals=()
if [[ -n "${CURSOR_API_KEY:-}" ]]; then
  agent_globals+=(--api-key "$CURSOR_API_KEY")
fi

# Worker options belong on `agent worker`, before the `start` subcommand.
worker_opts=(
  --name "$NAME"
  --worker-dir "$WORKER_DIR"
  --management-addr "$MGMT_ADDR"
  --data-dir "$DATA_DIR"
  --idle-release-timeout "$IDLE_TIMEOUT"
)

if [[ -n "$AUTH_TOKEN_FILE" ]]; then
  if [[ ! -f "$AUTH_TOKEN_FILE" ]]; then
    echo "error: auth token file missing: $AUTH_TOKEN_FILE" >&2
    exit 1
  fi
  worker_opts+=(--auth-token-file "$AUTH_TOKEN_FILE")
elif [[ -z "${CURSOR_API_KEY:-}" ]]; then
  echo "info: no CURSOR_API_KEY or auth token file; using local agent login session" >&2
fi

start_opts=()
if [[ "${CURSOR_APPLIANCE_DEBUG:-0}" == "1" ]]; then
  worker_opts+=(--debug)
  start_opts+=(--verbose)
fi

echo "Starting Cursor appliance worker"
echo "  name:     $NAME"
echo "  repo:     $WORKER_DIR"
echo "  health:   http://${MGMT_ADDR}/healthz"
echo "  idle:     ${IDLE_TIMEOUT}s (0 = never auto-release)"
echo "  agents:   https://cursor.com/agents"

exec agent "${agent_globals[@]}" worker "${worker_opts[@]}" start "${start_opts[@]}"
