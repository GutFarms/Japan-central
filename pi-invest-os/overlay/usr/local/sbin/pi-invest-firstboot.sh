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
  useradd -m -s /bin/bash -G sudo,gpio,i2c,spi,video,render,input pi 2>/dev/null \
    || useradd -m -s /bin/bash -G sudo,video,render,input pi
  echo 'pi:change-me' | chpasswd
  echo pi
}

TARGET_USER="$(detect_user)"
TARGET_UID="$(id -u "$TARGET_USER")"
TARGET_HOME="$(getent passwd "$TARGET_USER" | cut -d: -f6)"
echo "==> Operator user: $TARGET_USER ($TARGET_HOME) uid=$TARGET_UID"

# Ensure video/input groups for kiosk
usermod -aG video,render,input,sudo "$TARGET_USER" 2>/dev/null || true

echo "==> Waiting for network"
for _ in $(seq 1 90); do
  if getent hosts deb.debian.org >/dev/null 2>&1 || ping -c1 -W2 1.1.1.1 >/dev/null 2>&1; then
    break
  fi
  sleep 2
done

export DEBIAN_FRONTEND=noninteractive
echo "==> Installing system packages (agent + desktop + browser + auto-update)"
apt-get update -y
apt-get install -y --no-install-recommends \
  python3 python3-venv python3-pip python3-dev \
  git curl ca-certificates rsync build-essential libffi-dev \
  avahi-daemon \
  unattended-upgrades apt-listchanges \
  chromium \
  fonts-liberation fonts-dejavu-core \
  gvfs mousepad

echo "==> Installing Raspberry Pi desktop (Wayland / labwc)"
# Trixie metapackages (replaces legacy raspberrypi-ui-mods)
if ! apt-get install -y --no-install-recommends \
  rpd-wayland-core rpd-theme rpd-preferences rpd-utilities; then
  echo "WARN: rpd-wayland-core failed; trying XFCE fallback"
  apt-get install -y --no-install-recommends \
    xfce4 xfce4-terminal lightdm chromium || true
fi

# Optional extras (ignore failures on Lite repos)
apt-get install -y --no-install-recommends \
  rpd-applications rpd-graphics || true

# Keep cage available as optional fullscreen kiosk (disabled when desktop is on)
apt-get install -y --no-install-recommends cage seatd || true

# Enable unattended security updates
if [[ -f /etc/apt/apt.conf.d/50unattended-upgrades ]]; then
  cat >/etc/apt/apt.conf.d/20auto-upgrades <<'EOF'
APT::Periodic::Update-Package-Lists "1";
APT::Periodic::Unattended-Upgrade "1";
APT::Periodic::AutocleanInterval "7";
EOF
fi

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

# Ensure OS feature flags exist in .env
ensure_env() {
  local key="$1" val="$2"
  if ! grep -q "^${key}=" "$AGENT_ROOT/.env" 2>/dev/null; then
    printf '\n%s=%s\n' "$key" "$val" >>"$AGENT_ROOT/.env"
  fi
}
ensure_env PI_INVEST_AUTO_UPDATE true
ensure_env PI_INVEST_UPDATE_URL https://github.com/GutFarms/Japan-central.git
DEFAULT_BRANCH=master
if [[ -f /etc/pi-invest-os-update-branch ]]; then
  DEFAULT_BRANCH="$(tr -d '[:space:]' </etc/pi-invest-os-update-branch)"
fi
ensure_env PI_INVEST_UPDATE_BRANCH "$DEFAULT_BRANCH"
ensure_env PI_INVEST_KIOSK_URL http://127.0.0.1:8787
ensure_env PI_INVEST_DESKTOP true
ensure_env PI_INVEST_KIOSK false
chown "$TARGET_USER:$TARGET_USER" "$AGENT_ROOT/.env"
chmod 600 "$AGENT_ROOT/.env"

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

# Desktop launcher + autostart dashboard in a window
mkdir -p \
  "$TARGET_HOME/Desktop" \
  "$TARGET_HOME/.local/share/applications" \
  "$TARGET_HOME/.config/autostart"
cat >"$TARGET_HOME/.local/share/applications/pi-invest-dashboard.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Pi Invest Dashboard
Comment=Open the local Pi Invest dashboard
Exec=chromium --app=http://127.0.0.1:8787 --new-window
Icon=chromium
Terminal=false
Categories=Network;Finance;
StartupNotify=true
EOF
cp "$TARGET_HOME/.local/share/applications/pi-invest-dashboard.desktop" \
  "$TARGET_HOME/Desktop/" 2>/dev/null || true
# Autostart on login (waits briefly for the local dashboard service)
cat >"$TARGET_HOME/.config/autostart/pi-invest-dashboard.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Pi Invest Dashboard
Comment=Auto-open dashboard after login
Exec=/bin/bash -lc 'for i in \$(seq 1 60); do curl -fsS http://127.0.0.1:8787/api/health >/dev/null 2>&1 && break; sleep 2; done; exec chromium --app=http://127.0.0.1:8787 --new-window'
Icon=chromium
Terminal=false
X-GNOME-Autostart-enabled=true
EOF
chown -R "$TARGET_USER:$TARGET_USER" "$TARGET_HOME/Desktop" \
  "$TARGET_HOME/.local" "$TARGET_HOME/.config" 2>/dev/null || true
chmod +x "$TARGET_HOME/Desktop/pi-invest-dashboard.desktop" 2>/dev/null || true

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

cat > /etc/systemd/system/pi-invest-kiosk.service <<EOF
[Unit]
Description=Pi Invest dashboard kiosk (Chromium fullscreen)
After=pi-invest-dashboard.service network-online.target
Wants=pi-invest-dashboard.service
ConditionPathExists=/var/lib/pi-invest/firstboot-done
ConditionPathExists=/dev/dri/card0

[Service]
Type=simple
User=${TARGET_USER}
Group=${TARGET_USER}
SupplementaryGroups=video render input
PAMName=login
TTYPath=/dev/tty7
TTYReset=yes
TTYVHangup=yes
Environment=PYTHONUNBUFFERED=1
Environment=XDG_RUNTIME_DIR=/run/user/${TARGET_UID}
EnvironmentFile=-${AGENT_ROOT}/.env
ExecStartPre=+mkdir -p /run/user/${TARGET_UID}
ExecStartPre=+chown ${TARGET_UID}:${TARGET_UID} /run/user/${TARGET_UID}
ExecStartPre=+chmod 700 /run/user/${TARGET_UID}
ExecStart=/usr/local/sbin/pi-invest-kiosk.sh
Restart=on-failure
RestartSec=10

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

echo "==> Enabling services (agent, dashboard, desktop, auto-update)"
systemctl daemon-reload
systemctl enable pi-invest.service
systemctl enable pi-invest-dashboard.service
systemctl enable pi-invest-update.timer
systemctl enable unattended-upgrades.service 2>/dev/null || true
systemctl enable seatd.service 2>/dev/null || true

# Boot to graphical desktop with autologin (B4)
systemctl set-default graphical.target || true
if command -v raspi-config >/dev/null 2>&1; then
  raspi-config nonint do_boot_behaviour B4 || true
fi

# LightDM autologin fallback / reinforcement
if [[ -d /etc/lightdm ]]; then
  mkdir -p /etc/lightdm/lightdm.conf.d
  cat >/etc/lightdm/lightdm.conf.d/90-pi-invest.conf <<EOF
[Seat:*]
autologin-user=${TARGET_USER}
autologin-user-timeout=0
user-session=rpd-labwc
greeter-hide-users=false
EOF
  # If XFCE was the fallback, prefer that session when present
  if [[ -f /usr/share/wayland-sessions/xfce-wayland.desktop ]] \
    || [[ -f /usr/share/xsessions/xfce.desktop ]]; then
    sed -i 's/^user-session=.*/user-session=xfce-wayland/' \
      /etc/lightdm/lightdm.conf.d/90-pi-invest.conf || true
  fi
  systemctl enable lightdm.service 2>/dev/null || true
fi

# Desktop owns the display — keep cage kiosk off unless explicitly enabled
if grep -qiE '^PI_INVEST_KIOSK=(true|1)' "$AGENT_ROOT/.env" 2>/dev/null; then
  systemctl enable pi-invest-kiosk.service || true
else
  systemctl disable pi-invest-kiosk.service 2>/dev/null || true
  systemctl mask pi-invest-kiosk.service 2>/dev/null || true
fi

systemctl restart pi-invest.service || true
systemctl restart pi-invest-dashboard.service || true
systemctl start pi-invest-update.timer || true

hostnamectl set-hostname pi-invest || true
if ! grep -q 'pi-invest' /etc/hosts 2>/dev/null; then
  echo '127.0.1.1 pi-invest' >> /etc/hosts
fi

date -Is > "$DONE"
chmod 644 "$DONE"
systemctl disable pi-invest-firstboot.service || true

IP="$(hostname -I 2>/dev/null | awk '{print $1}')"
echo "==== First-boot complete ===="
echo "Desktop:   Raspberry Pi Wayland desktop with autologin as ${TARGET_USER}"
echo "Dashboard: opens automatically in Chromium; also on the Desktop"
echo "URL:       http://127.0.0.1:8787"
echo "Remote:    ssh -L 8787:127.0.0.1:8787 ${TARGET_USER}@${IP:-pi-invest.local}"
echo "Auto-update timer: systemctl status pi-invest-update.timer"
echo "Manual update:     sudo pi-invest-update.sh --force"
echo "Optional kiosk:    set PI_INVEST_KIOSK=true in .env then unmask/enable pi-invest-kiosk"
echo "Agent dir: $AGENT_ROOT"
echo "Secrets:   $AGENT_ROOT/.env"
