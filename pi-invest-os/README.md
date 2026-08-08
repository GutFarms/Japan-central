# Pi Invest OS

**Version 0.3** — a bootable Raspberry Pi OS image with the [Pi Invest Agent](../pi-invest-agent/) baked in, plus **Chromium kiosk** and **daily auto-update**.

Flash this onto a microSD (or NVMe), boot a **Raspberry Pi 5**, and the agent provisions itself on first boot: Python venv, paper trading loop, local dashboard, fullscreen browser, and update timer.

## What you get

| Item | Detail |
|---|---|
| Base OS | Raspberry Pi OS Lite **64-bit** (Trixie / arm64) |
| Hostname | `pi-invest` |
| Agent path | `/opt/pi-invest-agent` |
| Default user | `pi` / password `change-me` (change immediately) |
| SSH | Enabled |
| First boot | Installs deps, Chromium, cage kiosk, unattended-upgrades |
| Web browser | Chromium fullscreen kiosk on HDMI → `http://127.0.0.1:8787` |
| Auto-update | Daily timer pulls latest agent from GitHub + restarts services |
| OS security | `unattended-upgrades` enabled |
| Trading mode | Paper (live still double-gated) |
| Dashboard | `http://127.0.0.1:8787` |

## Flash the image

> **Raspberry Pi Imager tip:** Choose OS → **Use custom**, then set the file filter to **All files**. If `.img.xz` still does not show, extract it to `.img` with 7-Zip (Windows) or `xz -dk` (macOS/Linux). Full steps: [FLASH.md](./FLASH.md).

### Option A — download prebuilt (recommended)

**Release:** [Pi Invest OS v0.3.0](https://github.com/GutFarms/Japan-central/releases/tag/pi-invest-os-v0.3.0)

| File | Link |
|---|---|
| Image (~502 MB) | [pi-invest-os-0.3.0-arm64.img.xz](https://github.com/GutFarms/Japan-central/releases/download/pi-invest-os-v0.3.0/pi-invest-os-0.3.0-arm64.img.xz) |
| Checksums | [pi-invest-os-0.3.0-arm64.sha256](https://github.com/GutFarms/Japan-central/releases/download/pi-invest-os-v0.3.0/pi-invest-os-0.3.0-arm64.sha256) |

```bash
# Verify (optional)
sha256sum -c pi-invest-os-0.3.0-arm64.sha256

# Extract then flash with Imager (Use custom → select the .img)
xz -dk pi-invest-os-0.3.0-arm64.img.xz

# Or flash from CLI without Imager (Linux — replace sdX carefully)
xzcat pi-invest-os-0.3.0-arm64.img.xz | sudo dd of=/dev/sdX bs=4M status=progress conv=fsync
```

**Imager custom OS list** (Imager downloads the image for you):

```
https://raw.githubusercontent.com/GutFarms/Japan-central/cursor/pi-invest-os-0b6b/pi-invest-os/imager/os_list.json
```

### Option B — build it yourself

```bash
cd pi-invest-os
./scripts/build-image.sh
```

## First boot

1. Power on with HDMI attached (for the kiosk browser). Wait 5–15 minutes (apt installs Chromium).
2. Optional: edit `pi-invest.env` on the boot partition before first power-on.
3. On screen: Chromium opens the dashboard fullscreen.
4. SSH: `ssh pi@pi-invest.local` (password `change-me`).

```bash
sudo journalctl -u pi-invest-firstboot -u pi-invest -u pi-invest-kiosk -f
systemctl status pi-invest-update.timer
sudo pi-invest-update.sh --force   # manual agent update
```

## Auto-update

- **Agent:** `pi-invest-update.timer` runs daily (~04:15 local), clones `PI_INVEST_UPDATE_BRANCH` from GitHub, syncs `/opt/pi-invest-agent` (keeps `.env`, `config.yaml`, `data/`), reinstalls the package, restarts services.
- **Disable:** set `PI_INVEST_AUTO_UPDATE=false` in `/opt/pi-invest-agent/.env`.
- **OS packages:** `unattended-upgrades` applies security updates automatically.

## Browser / kiosk

- Service: `pi-invest-kiosk.service` (starts only if `/dev/dri/card0` exists).
- Compositor: Wayland `cage` + Chromium `--kiosk`.
- URL: `PI_INVEST_KIOSK_URL` (default `http://127.0.0.1:8787`).
- Desktop launcher also installed at `~/Desktop/pi-invest-dashboard.desktop`.
- Headless (no display): kiosk is skipped; use SSH tunnel for the dashboard.

## Layout

```
pi-invest-os/
  VERSION
  boot/pi-invest.env
  overlay/usr/local/sbin/
    pi-invest-firstboot.sh
    pi-invest-update.sh
    pi-invest-kiosk.sh
  overlay/etc/systemd/system/
    pi-invest*.service
    pi-invest-update.timer
  scripts/build-image.sh
```

## Safety

Paper by default. Live trading/transfers remain double-gated. Change the default password after first login. Not financial advice.

## License

MIT — use at your own risk.
