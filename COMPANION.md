# CYD Companion (Windows)

Native **C++** desktop app — the control interface for the ESP32-2432S028 miner.
The board does not run setup UI; configure everything here over USB.

## Download / build

```bash
./scripts/build-companion-windows.sh
```

Outputs:

- `dist/CYD-Companion-App-Only.zip` — exe only
- `dist/CYD-Companion-Portable.zip` — exe + README
- `dist/CYD-Companion-Setup.exe` — NSIS installer (if `makensis` available)

## First-time setup

1. Flash C++ firmware (`./scripts/build-flash-images.sh`)
2. Plug USB — board shows **Waiting for app**
3. Run `cyd-companion.exe` → pick COM → **Connect**
4. Enter WiFi + stratum + worker + pool password → **Save & reboot**

## Features

- Live stats from `cmp status` (H/s, pool, accepts, WiFi, CPU)
- WiFi / pool / CPU / hash-focus write via `cmp set`
- Clock apply + reboot
- Pool reconnect

## USB protocol

```text
cmp ping                 → CMP ok usb
cmp status               → CMPSTATUS {…}
cmp config               → CMPCONFIG {…,"configured":true|false}
cmp set auth=…&wifi_ssid=…&wifi_password=…&worker=…&stratum=…&password=…&cpu_mhz=240
cmp clock auth=…&cpu_mhz=240
cmp reboot auth=…
```

Unconfigured boards accept `cmp set` without auth. After save, Auth = pool password.
