#!/usr/bin/env bash
set -euo pipefail

UNIT_NAME=cursor-appliance.service

if [[ $EUID -ne 0 ]]; then
  echo "Run with sudo" >&2
  exit 1
fi

systemctl stop "$UNIT_NAME" 2>/dev/null || true
systemctl disable "$UNIT_NAME" 2>/dev/null || true
rm -f "/etc/systemd/system/$UNIT_NAME"
systemctl daemon-reload

echo "Removed $UNIT_NAME"
