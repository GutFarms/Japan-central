#!/usr/bin/env bash
# Quick local status for the Cursor appliance.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

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
MGMT_ADDR="${CURSOR_APPLIANCE_MANAGEMENT_ADDR:-127.0.0.1:8733}"
VERSION="$(tr -d '[:space:]' <"$ROOT/VERSION" 2>/dev/null || echo unknown)"

echo "Cursor appliance v${VERSION}"
echo "  name: ${NAME}"

if command -v agent >/dev/null 2>&1; then
  echo "  agent: $(agent --version 2>/dev/null || echo present)"
else
  echo "  agent: NOT FOUND"
fi

if systemctl list-unit-files cursor-appliance.service >/dev/null 2>&1; then
  echo "  service: $(systemctl is-active cursor-appliance.service 2>/dev/null || echo unknown)"
else
  echo "  service: not installed"
fi

if curl -fsS --max-time 2 "http://${MGMT_ADDR}/healthz" >/dev/null 2>&1; then
  echo "  healthz: OK (http://${MGMT_ADDR}/healthz)"
else
  echo "  healthz: unreachable (http://${MGMT_ADDR}/healthz)"
fi

if pgrep -af 'agent worker' >/dev/null 2>&1; then
  echo "  process:"
  pgrep -af 'agent worker' | sed 's/^/    /'
else
  echo "  process: not running"
fi
