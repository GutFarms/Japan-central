#!/usr/bin/env bash
# First-boot provisioning for Pi Invest OS (Raspberry Pi 5).
# Runs once after networking is up, then disables itself.
set -euo pipefail

LOG=/var/log/pi-invest-firstboot.log
DONE=/var/lib/pi-invest/firstboot-done
AGENT_ROOT=/opt/pi-invest-agent
BOOT_FIRMWARE=/boot/firmware
[[ -d "$BOOT_FIRMWARE" ]] || BOOT_FIRMWARE=/boot
BOOT_ENV="$BOOT_FIRMWARE/pi-invest.env"

exec > >(tee -a "$LOG") 2>&1
echo "==== Pi Invest first-boot $(date -Is) ===="

if [[ -f "$DONE" ]]; then
  echo "Already provisioned ($DONE); exiting"
  systemctl disable pi-invest-firstboot.service || true
  exit 0
fi

mkdir -p /var/lib/pi-invest /etc/systemd/system

detect_user() {
  if getent passwd pi >/dev/null 2>&1; then
    echo pi
    return
  fi
  local home_user
  home_user="$(ls -1 /home 2>/dev/null | head -1 || true)"
  if [[ -n "${home_user:-}" ]] && getent passwd "$home_user" >/dev/null 2>&1; then
    echo "$home_user"
    return
  fi
  # Create local operator account if the image has none yet
  useradd -m -s /bin/bash -G sudo,gpio,i2c,spi pi 2>/dev/null || useradd -m -s /bin/bash -G sudo pi
  echo 'pi:change-me' | chpasswd
  echo pi
}

TARGET_USER="$(detect_user)"
TARGET_HOME="$(getent passwd "$TARGET_USER" | cut -d: -f6)"
echo "==> Operator user: $TARGET_USER ($TARGET_HOME)"

echo "==> Waiting for network"
for _ in $(seq 1 90); do
  if getent hosts deb.debian.org >/dev/null 2>&1 || ping -c1 -W2 1.1.1.1 >/dev/null 2>&1; then
    break
  fi
  sleep 2
done

export DEBIAN_FRONTEND=noninteractive
echo "==> Installing system packages"
apt-get update -y
apt-get install -y --no-install-recommends \
  python3 python3-venv python3-pip python3-dev \
  git curl ca-certificates build-essential libffi-dev \
  avahi-daemon

if [[ ! -d "$AGENT_ROOT" ]]; then
  echo "ERROR: $AGENT_ROOT missing from image"
  exit 1
fi

# Import secrets / overrides from the boot partition (editable on any PC)
if [[ -f "$BOOT_ENV" ]]; then
  echo "==> Importing $BOOT_ENV"
  install -o "$TARGET_USER" -g "$TARGET_USER" -m 600 "$BOOT_ENV" "$AGENT_ROOT/.env"
else
  echo "==> No boot env found; using packaged defaults"
  if [[ ! -f "$AGENT_ROOT/.env" ]]; then
    install -o "$TARGET_USER" -g "$TARGET_USER" -m 600 \
      "$AGENT_ROOT/.env.example" "$AGENT_ROOT/.env"
  fi
fi

if [[ ! -f "$AGENT_ROOT/config/config.yaml" ]]; then
  if [[ -f "$AGENT_ROOT/config/config.os.yaml" ]]; then
    install -o "$TARGET_USER" -g "$TARGET_USER" -m 644 \
      "$AGENT_ROOT/config/config.os.yaml" "$AGENT_ROOT/config/config.yaml"
  else
    install -o "$TARGET_USER" -g "$TARGET_USER" -m 644 \
      "$AGENT_ROOT/config/config.example.yaml" "$AGENT_ROOT/config/config.yaml"
  fi
fi

chown -R "$TARGET_USER:$TARGET_USER" "$AGENT_ROOT"
mkdir -p "$AGENT_ROOT/data"
chown "$TARGET_USER:$TARGET_USER" "$AGENT_ROOT/data"

# Rewrite service units for this user / path
for unit in pi-invest.service pi-invest-dashboard.service; do
  src="$AGENT_ROOT/systemd/$unit"
  if [[ -f "$src" ]]; then
    sed \
      -e "s|User=pi|User=${TARGET_USER}|g" \
      -e "s|/opt/pi-invest-agent|${AGENT_ROOT}|g" \
      -e "s|/home/pi/Japan-central/pi-invest-agent|${AGENT_ROOT}|g" \
      "$src" > "/etc/systemd/system/$unit"
  fi
done

# Ensure ConditionPathExists units from overlay still work after rewrite
cat > /etc/systemd/system/pi-invest.service <<EOF
[Unit]
Description=Pi Invest Agent — autonomous income trader
After=network-online.target pi-invest-firstboot.service
Wants=network-online.target
ConditionPathExists=/var/lib/pi-invest/firstboot-done

[Service]
Type=simple
User=${TARGET_USER}
WorkingDirectory=${AGENT_ROOT}
Environment=PYTHONUNBUFFERED=1
EnvironmentFile=-${AGENT_ROOT}/.env
ExecStart=${AGENT_ROOT}/.venv/bin/pi-invest run
Restart=on-failure
RestartSec=30

[Install]
WantedBy=multi-user.target
EOF

cat > /etc/systemd/system/pi-invest-dashboard.service <<EOF
[Unit]
Description=Pi Invest Agent dashboard
After=network-online.target pi-invest.service pi-invest-firstboot.service
ConditionPathExists=/var/lib/pi-invest/firstboot-done

[Service]
Type=simple
User=${TARGET_USER}
WorkingDirectory=${AGENT_ROOT}
Environment=PYTHONUNBUFFERED=1
EnvironmentFile=-${AGENT_ROOT}/.env
ExecStart=${AGENT_ROOT}/.venv/bin/pi-invest dashboard
Restart=on-failure
RestartSec=20

[Install]
WantedBy=multi-user.target
EOF

echo "==> Creating Python venv + installing agent"
sudo -u "$TARGET_USER" bash -lc "
  set -euo pipefail
  cd '$AGENT_ROOT'
  python3 -m venv .venv
  . .venv/bin/activate
  pip install --upgrade pip wheel
  pip install -e .
"

echo "==> Smoke test (simulator preview)"
sudo -u "$TARGET_USER" bash -lc "
  cd '$AGENT_ROOT'
  . .venv/bin/activate
  pi-invest once --preview --simulator || true
"

echo "==> Enabling services"
systemctl daemon-reload
systemctl enable pi-invest.service
systemctl enable pi-invest-dashboard.service
systemctl restart pi-invest.service || true
systemctl restart pi-invest-dashboard.service || true

# Hostname convenience
hostnamectl set-hostname pi-invest || true
if ! grep -q 'pi-invest' /etc/hosts 2>/dev/null; then
  echo '127.0.1.1 pi-invest' >> /etc/hosts
fi

date -Is > "$DONE"
chmod 644 "$DONE"
systemctl disable pi-invest-firstboot.service || true

IP="$(hostname -I 2>/dev/null | awk '{print $1}')"
echo "==== First-boot complete ===="
echo "Dashboard: http://${IP:-127.0.0.1}:8787  (bind is localhost by default — use SSH tunnel or set dashboard.host)"
echo "Agent dir: $AGENT_ROOT"
echo "Secrets:   $AGENT_ROOT/.env  (seeded from $BOOT_ENV when present)"
