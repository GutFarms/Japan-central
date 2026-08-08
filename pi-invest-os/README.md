# Pi Invest OS

**Version 0.4** — bootable Raspberry Pi OS with a **full desktop environment**, the [Pi Invest Agent](../pi-invest-agent/), Chromium, and daily auto-update.

## What you get

| Item | Detail |
|---|---|
| Base OS | Raspberry Pi OS **64-bit** (Trixie) |
| Desktop | Official **Wayland / labwc** desktop with autologin |
| App menu | **Pi Invest Dashboard** local application (`pi-invest-dashboard`) |
| Browser | Chromium opens the dashboard from the menu / Desktop icon |
| Hostname | `pi-invest` |
| Agent | `/opt/pi-invest-agent` paper trading + local dashboard |
| Auto-update | Daily GitHub pull of the agent |
| Default user | `pi` / `change-me` (**change immediately**) |

## Flash

See **[FLASH.md](./FLASH.md)** if Raspberry Pi Imager does not show the `.img.xz` file.

**Release:** [Pi Invest OS v0.4.0](https://github.com/GutFarms/Japan-central/releases/tag/pi-invest-os-v0.4.0)

| File | Link |
|---|---|
| Image | [pi-invest-os-0.4.0-arm64.img.xz](https://github.com/GutFarms/Japan-central/releases/download/pi-invest-os-v0.4.0/pi-invest-os-0.4.0-arm64.img.xz) |
| Checksums | [pi-invest-os-0.4.0-arm64.sha256](https://github.com/GutFarms/Japan-central/releases/download/pi-invest-os-v0.4.0/pi-invest-os-0.4.0-arm64.sha256) |

```bash
xz -dk pi-invest-os-0.4.0-arm64.img.xz   # then Imager → Use custom → .img
```

Imager OS list JSON:
```
https://raw.githubusercontent.com/GutFarms/Japan-central/cursor/pi-invest-os-0b6b/pi-invest-os/imager/os_list.json
```

## First boot

1. HDMI monitor + keyboard/mouse recommended.
2. Wait **10–25 minutes** (desktop packages download on first boot).
3. Autologin to the desktop; Chromium opens the dashboard.
4. Change password: `passwd`

Optional fullscreen kiosk instead of desktop window: set `PI_INVEST_KIOSK=true` in `.env`, then:
```bash
sudo systemctl unmask pi-invest-kiosk.service
sudo systemctl enable --now pi-invest-kiosk.service
```

## Build yourself

```bash
cd pi-invest-os && ./scripts/build-image.sh
```

## Safety

Paper trading by default. Not financial advice. Change the default password after first login.

## License

MIT
