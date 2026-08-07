# Pi Invest OS

**Version 0.2** — a bootable Raspberry Pi OS image with the [Pi Invest Agent](../pi-invest-agent/) baked in.

Flash this onto a microSD (or NVMe), boot a **Raspberry Pi 5**, and the agent provisions itself on first boot: Python venv, paper trading loop, and local dashboard.

## What you get

| Item | Detail |
|---|---|
| Base OS | Raspberry Pi OS Lite **64-bit** (Trixie / arm64) |
| Hostname | `pi-invest` |
| Agent path | `/opt/pi-invest-agent` |
| Default user | `pi` / password `change-me` (change immediately) |
| SSH | Enabled |
| First boot | Installs deps, creates venv, enables `pi-invest` + dashboard |
| Trading mode | Paper (live still double-gated) |
| Dashboard | `http://127.0.0.1:8787` after first boot (SSH tunnel recommended) |

## Flash the image

### Option A — download prebuilt (recommended)

**Release:** [Pi Invest OS v0.2.0](https://github.com/GutFarms/Japan-central/releases/tag/pi-invest-os-v0.2.0)

| File | Link |
|---|---|
| Image (~501 MB) | [pi-invest-os-0.2.0-arm64.img.xz](https://github.com/GutFarms/Japan-central/releases/download/pi-invest-os-v0.2.0/pi-invest-os-0.2.0-arm64.img.xz) |
| Checksums | [pi-invest-os-0.2.0-arm64.sha256](https://github.com/GutFarms/Japan-central/releases/download/pi-invest-os-v0.2.0/pi-invest-os-0.2.0-arm64.sha256) |

```bash
# Verify (optional)
sha256sum -c pi-invest-os-0.2.0-arm64.sha256

# Linux — replace sdX with your SD/NVMe device (not a partition)
xzcat pi-invest-os-0.2.0-arm64.img.xz | sudo dd of=/dev/sdX bs=4M status=progress conv=fsync
```

Or open the `.img.xz` in [Raspberry Pi Imager](https://www.raspberrypi.com/software/) → **Use custom**.

### Option B — build it yourself

On a Linux amd64/arm64 host with sudo:

```bash
cd pi-invest-os
./scripts/build-image.sh
# writes dist/pi-invest-os-<version>-arm64.img.xz
```

The builder downloads Raspberry Pi OS Lite, expands the rootfs, injects the agent + first-boot service, enables SSH, and seeds boot-partition config.

## First boot

1. Insert the card, power on the Pi 5, wait 3–10 minutes (apt + pip on first boot).
2. Optional: before first power-on, mount the boot partition on any PC and edit `pi-invest.env` (API keys, dashboard password, ntfy topic).
3. SSH in:

```bash
ssh pi@pi-invest.local
# password: change-me
```

4. Check status:

```bash
sudo journalctl -u pi-invest-firstboot -u pi-invest -f
pi-invest status
```

5. Dashboard via SSH tunnel (default bind is localhost):

```bash
ssh -L 8787:127.0.0.1:8787 pi@pi-invest.local
# open http://127.0.0.1:8787  (user pi / password from pi-invest.env)
```

## Layout

```
pi-invest-os/
  VERSION                 # 0.2.x
  boot/                   # files placed on the FAT boot partition
    pi-invest.env         # secrets template (copied to agent .env on first boot)
    userconf.txt          # default pi user
  config/config.os.yaml   # paper-first OS defaults
  overlay/                # rootfs overlay (hostname, motd, systemd, firstboot)
  scripts/build-image.sh  # reproducible image builder
  dist/                   # build output (gitignored)
```

## Relationship to `pi-invest-agent`

| | `pi-invest-agent` (v0.1) | `pi-invest-os` (v0.2) |
|---|---|---|
| Install | `pip` / git clone on an existing Pi OS | Flash a full OS image |
| Audience | Developers iterating on the agent | Appliance-style deploy |
| Code | Source of truth | Build copies agent into `/opt` |

Secrets are **not** baked into the golden image beyond a change-me dashboard password. Put real keys in `pi-invest.env` on the boot partition or edit `/opt/pi-invest-agent/.env` after login.

## Safety

Same gates as the agent package: paper by default, `ALLOW_LIVE_TRADING` / `ALLOW_LIVE_TRANSFERS` required for live, kill switch, allowlist, send confirmation. This image is for experimentation — not financial advice.

## License

MIT — use at your own risk.
