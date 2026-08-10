# CYD Monitor — downloadable host app

Auto-connects to an ESP32-CYD over USB, shows **speedometer dials**, and streams PC/GPU metrics in the background.

## Download

| File | Platform |
| --- | --- |
| `CYD-Monitor-portable.zip` | Windows / Linux / macOS |
| `CYD-Monitor` | Linux standalone (no Python) |

### Windows
1. Unzip `CYD-Monitor-portable.zip`
2. Double-click **`CYD Monitor.bat`**
3. First run installs a local `.venv` (needs [Python 3](https://www.python.org/downloads/))
4. App opens **without a terminal window** (`pythonw`)

### Linux
```bash
chmod +x CYD-Monitor
./CYD-Monitor
# or from the portable zip:
./CYD-Monitor.sh
```

## Features
- Speedometer dials: large CPU + smaller GPU / RAM with glow arcs and sparklines
- CPU temperature card, OC/clock panel (CPU current/min/max, GPU core/mem clocks)
- Disk space bar (used / total / free GB) and VRAM usage bar
- Extra PC stats: swap, network Mbps, boost %, uptime
- **Settings** tab: port, baud, interval, host label, optional UDP, tray behavior
- **Control the CYD from the app:** Flip 180°, brightness slider (release to apply), sync from device
- Minimize / close to **system tray** (background streaming)
- Window is freely resizable
- Auto USB detect + reconnect

## Pair with firmware
Flash `firmware/release/esp32-cyd-pc-monitor-merged.bin`, plug USB, launch the app.
