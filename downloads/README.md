# Downloads — CYD PC/GPU Monitor

Grab a zip, flash the board, run the app.

| Package | What it is |
| --- | --- |
| **[CYD-Monitor-bundle.zip](CYD-Monitor-bundle.zip)** | **Recommended** — firmware + portable PC app |
| **[CYD-Firmware.zip](CYD-Firmware.zip)** | Flash images only (`merged.bin` @ `0x0`) |
| **[CYD-Monitor-portable.zip](CYD-Monitor-portable.zip)** | PC app (Windows / Linux / macOS, needs Python) |
| **[CYD-Monitor-linux](CYD-Monitor-linux)** | Linux standalone binary (no Python) |

App monitor view includes CPU temp, OC/clock readouts, disk used/free, VRAM bar, and upgraded dial graphics.

## Quick start

1. Unzip **CYD-Monitor-bundle.zip**
2. Flash `firmware/esp32-cyd-pc-monitor-merged.bin` at address `0x0`:

```bash
pip install esptool
esptool.py --chip esp32 --port COMx --baud 921600 \
  write_flash 0x0 esp32-cyd-pc-monitor-merged.bin
```

3. Run `app/CYD-Monitor/CYD Monitor.bat` (Windows) or `./CYD-Monitor.sh` (Linux/macOS)
4. Leave the CYD plugged in over USB

## GitHub direct links

On the feature branch (until merged):

- Bundle: `https://github.com/GutFarms/Japan-central/raw/cursor/esp32-cyd-pc-monitor-9f0c/downloads/CYD-Monitor-bundle.zip`
- Firmware: `https://github.com/GutFarms/Japan-central/raw/cursor/esp32-cyd-pc-monitor-9f0c/downloads/CYD-Firmware.zip`
- Portable app: `https://github.com/GutFarms/Japan-central/raw/cursor/esp32-cyd-pc-monitor-9f0c/downloads/CYD-Monitor-portable.zip`
