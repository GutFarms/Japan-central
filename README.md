# ESP32-CYD PC / GPU Monitor

Firmware and host agent for an **ESP32 Cheap Yellow Display (CYD)** that shows live PC and GPU performance on a blue-hued landscape HUD.

```
╭────────── CYD  DESKTOP     ● LIVE ──────╮
│  ( large CPU dial )   (GPU)             │
│    0··50··100            71% / 68C      │
│         42%           (RAM)             │
│                          58%            │
╰──[USB] [VRAM 44%] [DISK 62%] [8.5 Mb]───╯
```

## Downloads

Ready-to-use packages (no build required):

| Package | Download |
| --- | --- |
| **Everything (recommended)** | [`downloads/CYD-Monitor-bundle.zip`](downloads/CYD-Monitor-bundle.zip) |
| Firmware flash images | [`downloads/CYD-Firmware.zip`](downloads/CYD-Firmware.zip) |
| PC monitor app (portable) | [`downloads/CYD-Monitor-portable.zip`](downloads/CYD-Monitor-portable.zip) |
| Linux monitor binary | [`downloads/CYD-Monitor-linux`](downloads/CYD-Monitor-linux) |
| **JC Time egui clock (Linux)** | [`downloads/JC-Time-linux`](downloads/JC-Time-linux) / [`zip`](downloads/JC-Time-linux.zip) |

**Quick start (CYD):** unzip the bundle → flash `firmware/esp32-cyd-pc-monitor-merged.bin` at `0x0` → run the app → plug USB.

```bash
pip install esptool
esptool.py --chip esp32 --port COMx --baud 921600 \
  write_flash 0x0 esp32-cyd-pc-monitor-merged.bin
```

**JC Time:** `chmod +x downloads/JC-Time-linux && ./downloads/JC-Time-linux` — Japan + Central dual egui clocks.

See [`downloads/README.md`](downloads/README.md) for details.

## What's in this workspace

| Path | Purpose |
| --- | --- |
| `downloads/` | **Downloadable** firmware + app zip packages |
| `clock/` | **egui** Japan & Central Time dual world clock |
| `firmware/` | PlatformIO project for ESP32-2432S028 (ILI9341, 320×240) |
| `host/` | Python agent that samples CPU/RAM/GPU and sends over USB serial and/or UDP |
| `protocol/` | Wire-format docs shared by both sides |

## Hardware

- ESP32-CYD (**ESP32-2432S028**), 2.8" ILI9341 + backlight on GPIO 21
- PC linked by **USB cable** (serial) and/or the same Wi‑Fi LAN
- NVIDIA GPU optional via NVML

## Firmware setup

1. Install [PlatformIO](https://platformio.org/) (CLI or VS Code / Cursor extension).
2. Wi‑Fi is set in `firmware/include/secrets.h` (SSID: `Stargate Command`). Edit that file if your network changes.

3. Build and flash:

```bash
cd firmware
pio run -t upload
pio device monitor
```

### Prebuilt `.bin` files

Flashable images are in [`firmware/release/`](firmware/release/):

- **`esp32-cyd-pc-monitor-merged.bin`** — flash at address `0x0` (bootloader + partitions + app)
- **`esp32-cyd-pc-monitor.bin`** — app only at `0x10000`

```bash
esptool.py --chip esp32 --port /dev/ttyUSB0 --baud 921600 \
  write_flash 0x0 firmware/release/esp32-cyd-pc-monitor-merged.bin
```

See [`firmware/release/FLASH.md`](firmware/release/FLASH.md) for details.

On boot the display is ready for **USB serial @ 115200** immediately; Wi‑Fi/UDP is optional in the background.

## Host application (auto USB)

Downloadable app with **speedometer dials**, a **Settings** tab (including **flip CYD screen** / brightness), and **tray / background** mode. It auto-detects the CYD over USB (no terminal window on Windows):

| File | Platform |
| --- | --- |
| [`host/release/CYD-Monitor-portable.zip`](host/release/CYD-Monitor-portable.zip) | Windows / Linux / macOS (Python launcher) |
| [`host/release/CYD-Monitor`](host/release/CYD-Monitor) | Linux standalone binary |

- **Windows:** unzip the portable zip → double-click `CYD Monitor.bat`
- **Linux:** `chmod +x CYD-Monitor && ./CYD-Monitor` (or use `CYD-Monitor.sh` from the zip)

See [`host/release/README.md`](host/release/README.md).

### CLI agent

```bash
cd host
python3 -m venv .venv
source .venv/bin/activate   # Windows: .venv\Scripts\activate
pip install -r requirements.txt
```

### USB serial (CLI)

Plug the CYD into USB, then:

```bash
python agent.py --list-ports
python agent.py --serial COM3          # Windows
python agent.py --serial /dev/ttyUSB0  # Linux
python agent.py --serial auto          # best CYD-like port
python desktop_app.py                  # GUI auto-connect app
```

### Wi‑Fi UDP

```bash
python agent.py --host 192.168.1.50 --port 4210
```

Both at once: `python agent.py --serial auto --host 192.168.1.50`

Useful flags:

- `--interval 0.5` — send rate (seconds)
- `--name RIG` — short label on the HUD
- `--gpu-index 0` — NVIDIA adapter index
- `--once` — single packet (smoke test)
- `--baud 115200` — serial baud rate

GPU fields require an NVIDIA driver + `nvidia-ml-py`. Without a supported GPU, CPU/RAM still update and GPU/VRAM stay at 0.

For UI bring-up without real sensors:

```bash
python simulate_demo.py --serial auto
python simulate_demo.py --host 192.168.1.50
```

## Protocol

See [`protocol/metrics_v1.md`](protocol/metrics_v1.md).

## UI theme

Dark/light blue speedometer dials with eased needles (sprite-composited on device). Host↔CYD link uses sequenced packets, ACK/RTT, and delta updates for a cleaner USB/Wi‑Fi stream.

## Project layout

```
firmware/
  platformio.ini
  include/     theme, metrics, wifi/serial config, GUI API
  src/         main, GUI, JSON parser, serial link
  release/     prebuilt .bin flash images
host/
  desktop_app.py           GUI auto-connect application
  agent.py                 CLI agent
  cyd_core.py              shared metrics + USB detect
  CYD Monitor.bat          Windows launcher
  CYD-Monitor.sh           Linux/macOS launcher
  build_app.sh             build standalone + portable zip
  release/                 downloadable app packages
protocol/
  metrics_v1.md
scripts/
  verify.sh        full rebuild + smoke checks
  smoke_host.py    offline host/protocol smoke test
```

## Verify

```bash
./scripts/verify.sh
```
