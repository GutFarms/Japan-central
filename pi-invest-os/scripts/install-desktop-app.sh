#!/usr/bin/env bash
# Install / refresh the Pi Invest Dashboard as a local desktop application.
# Safe to run on an already-provisioned Pi Invest OS install.
set -euo pipefail

if [[ $EUID -ne 0 ]]; then
  exec sudo -E bash "$0" "$@"
fi

TARGET_USER="${SUDO_USER:-pi}"
TARGET_HOME="$(getent passwd "$TARGET_USER" | cut -d: -f6)"
[[ -n "$TARGET_HOME" ]] || TARGET_HOME="/home/$TARGET_USER"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# When installed on device, scripts live under /usr/local; overlay may be in repo checkout
OVERLAY_BIN="$ROOT/overlay/usr/local/bin/pi-invest-dashboard"
OVERLAY_DESKTOP="$ROOT/overlay/usr/share/applications/pi-invest-dashboard.desktop"

install -d -m 755 /usr/local/bin /usr/share/applications

if [[ -f "$OVERLAY_BIN" ]]; then
  install -m 755 "$OVERLAY_BIN" /usr/local/bin/pi-invest-dashboard
elif [[ ! -x /usr/local/bin/pi-invest-dashboard ]]; then
  cat >/usr/local/bin/pi-invest-dashboard <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
URL="${PI_INVEST_KIOSK_URL:-http://127.0.0.1:8787}"
systemctl start pi-invest-dashboard.service 2>/dev/null || true
for _ in $(seq 1 45); do
  curl -fsS --max-time 2 "$URL/api/health" >/dev/null 2>&1 && break
  sleep 1
done
BROWSER="$(command -v chromium || command -v chromium-browser || command -v firefox || command -v xdg-open)"
exec "$BROWSER" --new-window --app="$URL"
EOF
  chmod 755 /usr/local/bin/pi-invest-dashboard
fi

if [[ -f "$OVERLAY_DESKTOP" ]]; then
  install -m 644 "$OVERLAY_DESKTOP" /usr/share/applications/pi-invest-dashboard.desktop
else
  cat >/usr/share/applications/pi-invest-dashboard.desktop <<'EOF'
[Desktop Entry]
Type=Application
Name=Pi Invest Dashboard
Comment=Open the local Pi Invest agent dashboard
Exec=/usr/local/bin/pi-invest-dashboard
Icon=utilities-system-monitor
Terminal=false
Categories=Network;Finance;Office;
StartupNotify=true
EOF
fi

# User menu + Desktop shortcut
install -d -m 755 \
  "$TARGET_HOME/.local/share/applications" \
  "$TARGET_HOME/Desktop" \
  "$TARGET_HOME/.config/autostart"
install -m 644 /usr/share/applications/pi-invest-dashboard.desktop \
  "$TARGET_HOME/.local/share/applications/pi-invest-dashboard.desktop"
install -m 755 /usr/share/applications/pi-invest-dashboard.desktop \
  "$TARGET_HOME/Desktop/pi-invest-dashboard.desktop"
install -m 644 /usr/share/applications/pi-invest-dashboard.desktop \
  "$TARGET_HOME/.config/autostart/pi-invest-dashboard.desktop"

# Mark Desktop launcher as trusted (Raspberry Pi / GNOME / labwc)
if command -v gio >/dev/null 2>&1; then
  sudo -u "$TARGET_USER" gio set \
    "$TARGET_HOME/Desktop/pi-invest-dashboard.desktop" \
    metadata::trusted true 2>/dev/null || true
fi
# pcmanfm / some Pi desktops use this extended attribute
if command -v setfattr >/dev/null 2>&1; then
  setfattr -n user.xdg.trusted -v true \
    "$TARGET_HOME/Desktop/pi-invest-dashboard.desktop" 2>/dev/null || true
fi

chown -R "$TARGET_USER:$TARGET_USER" \
  "$TARGET_HOME/.local/share/applications" \
  "$TARGET_HOME/Desktop/pi-invest-dashboard.desktop" \
  "$TARGET_HOME/.config/autostart" 2>/dev/null || true

update-desktop-database /usr/share/applications 2>/dev/null || true
gtk-update-icon-cache -f /usr/share/icons/hicolor 2>/dev/null || true

cat >/etc/sudoers.d/pi-invest-dashboard <<EOF
${TARGET_USER} ALL=(root) NOPASSWD: /usr/bin/systemctl start pi-invest-dashboard.service, /usr/bin/systemctl restart pi-invest-dashboard.service, /bin/systemctl start pi-invest-dashboard.service, /bin/systemctl restart pi-invest-dashboard.service
EOF
chmod 440 /etc/sudoers.d/pi-invest-dashboard

echo "Installed local app: Pi Invest Dashboard"
echo "  Menu:    Applications → Pi Invest Dashboard"
echo "  Desktop: $TARGET_HOME/Desktop/pi-invest-dashboard.desktop"
echo "  CLI:     pi-invest-dashboard"
echo "  URL:     http://127.0.0.1:8787"
