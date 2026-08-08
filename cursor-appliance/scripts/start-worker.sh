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

NAME="${CURSOR_APPLIANCE_NAME:-cursor-appliance}"
WORKER_DIR="${CURSOR_APPLIANCE_WORKER_DIR:-$REPO_ROOT}"
MGMT_ADDR="${CURSOR_APPLIANCE_MANAGEMENT_ADDR:-127.0.0.1:8733}"
DATA_DIR="${CURSOR_APPLIANCE_DATA_DIR:-$ROOT/data}"
AUTH_TOKEN_FILE="${CURSOR_APPLIANCE_AUTH_TOKEN_FILE:-}"

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

args=(
  worker start
  --name "$NAME"
  --worker-dir "$WORKER_DIR"
  --management-addr "$MGMT_ADDR"
  --data-dir "$DATA_DIR"
)

if [[ -n "$AUTH_TOKEN_FILE" ]]; then
  if [[ ! -f "$AUTH_TOKEN_FILE" ]]; then
    echo "error: auth token file missing: $AUTH_TOKEN_FILE" >&2
    exit 1
  fi
  args+=(--auth-token-file "$AUTH_TOKEN_FILE")
elif [[ -n "${CURSOR_API_KEY:-}" ]]; then
  args+=(--api-key "$CURSOR_API_KEY")
else
  # Fall back to stored browser login / agent session on this machine.
  echo "info: no CURSOR_API_KEY or auth token file; using local agent login session" >&2
fi

if [[ "${CURSOR_APPLIANCE_DEBUG:-0}" == "1" ]]; then
  args+=(--debug)
fi

echo "Starting Cursor appliance worker"
echo "  name:     $NAME"
echo "  repo:     $WORKER_DIR"
echo "  health:   http://${MGMT_ADDR}/healthz"
echo "  agents:   https://cursor.com/agents"

exec agent "${args[@]}"
