#!/usr/bin/env bash
# Install the Cursor appliance as a long-lived systemd service.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
UNIT_DIR=/etc/systemd/system
UNIT_NAME=cursor-appliance.service

if [[ $EUID -ne 0 ]]; then
  echo "Run with sudo" >&2
  exit 1
fi

TARGET_USER="${SUDO_USER:-${CURSOR_APPLIANCE_USER:-}}"
if [[ -z "$TARGET_USER" || "$TARGET_USER" == "root" ]]; then
  echo "error: refuse to run the worker as root; invoke via sudo from your user account" >&2
  exit 1
fi

TARGET_HOME="$(getent passwd "$TARGET_USER" | cut -d: -f6)"
if [[ -z "$TARGET_HOME" ]]; then
  echo "error: could not resolve home for user $TARGET_USER" >&2
  exit 1
fi

# Prefer user-local agent installs.
AGENT_BIN=""
for candidate in \
  "$TARGET_HOME/.local/bin/agent" \
  "$TARGET_HOME/.cursor/bin/agent" \
  /usr/local/bin/agent \
  /usr/bin/agent; do
  if [[ -x "$candidate" ]]; then
    AGENT_BIN="$candidate"
    break
  fi
done

if [[ -z "$AGENT_BIN" ]]; then
  echo "error: agent CLI not found for $TARGET_USER" >&2
  echo "Ask $TARGET_USER to run: $ROOT/scripts/setup.sh" >&2
  exit 1
fi

if [[ ! -f "$ROOT/.env" ]]; then
  echo "error: missing $ROOT/.env — copy .env.example and set CURSOR_API_KEY (or use agent login)" >&2
  exit 1
fi

# Harden credentials file for the service user.
chown "$TARGET_USER:$TARGET_USER" "$ROOT/.env"
chmod 600 "$ROOT/.env"

AGENT_BIN_DIR="$(dirname "$AGENT_BIN")"
PATH_VALUE="${AGENT_BIN_DIR}:${TARGET_HOME}/.local/bin:${TARGET_HOME}/.cursor/bin:/usr/local/bin:/usr/bin:/bin"

cat >"$UNIT_DIR/$UNIT_NAME" <<EOF
[Unit]
Description=Cursor appliance — local My Machines worker
Documentation=https://cursor.com/docs/cloud-agent/self-hosted-guides/my-machines
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=${TARGET_USER}
Group=${TARGET_USER}
WorkingDirectory=${ROOT}
Environment=HOME=${TARGET_HOME}
Environment=PATH=${PATH_VALUE}
EnvironmentFile=-${ROOT}/.env
ExecStart=${ROOT}/scripts/start-worker.sh
Restart=on-failure
RestartSec=15
KillMode=mixed
TimeoutStopSec=30
Nice=5
NoNewPrivileges=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
EOF

chmod 644 "$UNIT_DIR/$UNIT_NAME"
systemctl daemon-reload
systemctl enable "$UNIT_NAME"
systemctl restart "$UNIT_NAME"

echo "Enabled $UNIT_NAME for user ${TARGET_USER}"
echo "Status:  systemctl status $UNIT_NAME"
echo "Logs:    journalctl -u cursor-appliance -f"
echo "Health:  curl -fsS http://127.0.0.1:8733/healthz"
echo "Agents:  https://cursor.com/agents  (select cursor-appliance)"
