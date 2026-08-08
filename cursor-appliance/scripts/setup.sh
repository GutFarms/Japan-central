#!/usr/bin/env bash
# Bootstrap a lightweight local Cursor appliance (My Machines worker).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO_ROOT="$(cd "$ROOT/.." && pwd)"
VERSION="$(tr -d '[:space:]' <"$ROOT/VERSION")"

echo "==> Cursor appliance setup (v${VERSION})"
echo "    repo: $REPO_ROOT"

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "error: required command not found: $1" >&2
    exit 1
  }
}

need_cmd curl
need_cmd git

if [[ ! -d "$REPO_ROOT/.git" ]]; then
  echo "error: expected Japan-central checkout at $REPO_ROOT" >&2
  exit 1
fi

echo "==> Installing / refreshing Cursor Agent CLI"
curl -fsSL https://cursor.com/install | bash

# Ensure common install locations are on PATH for this script.
export PATH="${HOME}/.local/bin:${HOME}/.cursor/bin:${PATH}"

if ! command -v agent >/dev/null 2>&1; then
  echo "error: agent CLI still not on PATH after install" >&2
  echo "Add ~/.local/bin (or the installer's reported bin dir) to PATH, then re-run." >&2
  exit 1
fi

echo "    agent: $(agent --version 2>/dev/null || echo unknown)"

if [[ ! -f "$ROOT/.env" ]]; then
  cp "$ROOT/.env.example" "$ROOT/.env"
  echo "==> Created $ROOT/.env"
  echo "    Put your personal API key in CURSOR_API_KEY, or run: agent login"
fi

if [[ ! -f "$ROOT/config/appliance.env" ]]; then
  cp "$ROOT/config/appliance.env.example" "$ROOT/config/appliance.env"
fi

mkdir -p "$ROOT/data"

echo "==> Smoke check (worker preflight)"
if [[ -n "${CURSOR_API_KEY:-}" ]] || grep -qE '^CURSOR_API_KEY=.+' "$ROOT/.env" 2>/dev/null; then
  # shellcheck disable=SC1091
  set -a && source "$ROOT/.env" && set +a
  # `worker debug` is preflight-only; it does not stay attached like `worker start`.
  agent worker debug --name "${CURSOR_APPLIANCE_NAME:-cursor-appliance}" \
    --worker-dir "$REPO_ROOT" || true
else
  echo "    Skipping authenticated preflight (no API key yet)."
  echo "    After login or key setup, run: agent worker debug"
fi

chmod +x \
  "$ROOT/scripts/setup.sh" \
  "$ROOT/scripts/start-worker.sh" \
  "$ROOT/scripts/install_service.sh" \
  "$ROOT/scripts/uninstall_service.sh" \
  "$ROOT/scripts/status.sh"

echo
echo "Done. Next steps:"
echo "  1) Edit credentials:"
echo "       nano $ROOT/.env"
echo "     or: agent login"
echo "  2) Foreground test:"
echo "       $ROOT/scripts/start-worker.sh"
echo "  3) Or install as a boot service:"
echo "       sudo $ROOT/scripts/install_service.sh"
echo "  4) Pick \"cursor-appliance\" on https://cursor.com/agents"
echo
echo "Docs: https://cursor.com/docs/cloud-agent/self-hosted-guides/my-machines"
