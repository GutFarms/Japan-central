#!/usr/bin/env bash
# Run ON the Raspberry Pi to install the local Dashboard app immediately.
set -euo pipefail
sudo tee /usr/local/bin/pi-invest-dashboard >/dev/null <<'BIN'
#!/usr/bin/env bash
set -euo pipefail
URL="${PI_INVEST_KIOSK_URL:-http://127.0.0.1:8787}"
if [[ -f /opt/pi-invest-agent/.env ]]; then set -a; source /opt/pi-invest-agent/.env || true; set +a; URL="${PI_INVEST_KIOSK_URL:-$URL}"; fi
if ! curl -fsS --max-time 2 "$URL/api/health" >/dev/null 2>&1; then
  sudo -n /usr/bin/systemctl start pi-invest-dashboard.service 2>/dev/null || sudo /usr/bin/systemctl start pi-invest-dashboard.service || true
  for _ in $(seq 1 45); do curl -fsS --max-time 2 "$URL/api/health" >/dev/null 2>&1 && break; sleep 1; done
fi
BROWSER="$(command -v chromium || command -v chromium-browser || command -v xdg-open)"
exec "$BROWSER" --new-window --app="$URL"
BIN
sudo chmod 755 /usr/local/bin/pi-invest-dashboard
sudo tee /usr/share/applications/pi-invest-dashboard.desktop >/dev/null <<'DESK'
[Desktop Entry]
Type=Application
Name=Pi Invest Dashboard
Comment=Open the local Pi Invest agent dashboard
Exec=/usr/local/bin/pi-invest-dashboard
Icon=utilities-system-monitor
Terminal=false
Categories=Network;Finance;Office;
StartupNotify=true
DESK
USER_NAME="${USER:-pi}"
HOME_DIR="$(getent passwd "$USER_NAME" | cut -d: -f6)"
mkdir -p "$HOME_DIR/Desktop" "$HOME_DIR/.local/share/applications" "$HOME_DIR/.config/autostart"
cp /usr/share/applications/pi-invest-dashboard.desktop "$HOME_DIR/.local/share/applications/"
cp /usr/share/applications/pi-invest-dashboard.desktop "$HOME_DIR/Desktop/"
cp /usr/share/applications/pi-invest-dashboard.desktop "$HOME_DIR/.config/autostart/"
chmod +x "$HOME_DIR/Desktop/pi-invest-dashboard.desktop"
gio set "$HOME_DIR/Desktop/pi-invest-dashboard.desktop" metadata::trusted true 2>/dev/null || true
echo "$USER_NAME ALL=(root) NOPASSWD: /usr/bin/systemctl start pi-invest-dashboard.service, /usr/bin/systemctl restart pi-invest-dashboard.service" | sudo tee /etc/sudoers.d/pi-invest-dashboard >/dev/null
sudo chmod 440 /etc/sudoers.d/pi-invest-dashboard
sudo systemctl enable --now pi-invest-dashboard.service || true
sudo update-desktop-database /usr/share/applications 2>/dev/null || true
echo "Done. Open Applications menu → Pi Invest Dashboard  (or run: pi-invest-dashboard)"
