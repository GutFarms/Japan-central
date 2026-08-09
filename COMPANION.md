# CYD Companion (Windows)

GPU desktop app (`cyd-companion.exe`) for the ESP32-2432S028 scrypt miner.

## Download / build

Prebuilt zip: `dist/cyd-companion-windows.zip`

```bash
./scripts/build-companion-windows.sh
```

Or locally on Windows (MSVC/GNU):

```bash
cargo build --no-default-features --features companion --bin cyd-companion --release
```

Release builds on Windows use the `windows` subsystem — **no console/terminal window** on launch. Debug builds still show a console for logs.

## Features

| Tab | What it does |
|-----|----------------|
| Dashboard | Live hashrate, pool, WiFi, CPU MHz |
| Settings | WiFi / stratum / worker / password → flash |
| Overclock | CPU 80 / 160 / 240 MHz (soft-reset apply) |
| Discover | Probe LAN `/probe` for SCRYPT-CYD boards |

## Connection modes

### USB (default)

Plug the CYD USB cable into the PC (same CH340 COM port used for flashing).

1. Select **USB**
2. Pick the COM port → **Refresh** if needed
3. **Connect** (115200 baud)
4. Enter the **pool password** for Settings / Overclock writes

No Wi‑Fi required for status, config, save, clock, or reboot.

USB protocol (device replies with one line):

```text
cmp ping                 → CMP ok usb
cmp status               → CMPSTATUS {…}
cmp config               → CMPCONFIG {…}
cmp set auth=…&wifi_ssid=…&…
cmp clock auth=…&cpu_mhz=240
cmp reboot auth=…
```

### LAN (HTTP)

When the miner is already on Wi‑Fi:

1. Select **LAN**
2. Enter `http://<miner-ip>/` (or just the IP)
3. Enter pool password → **Connect**

Auth for writes = **pool password** (same as the board web UI).

## Board APIs used (LAN)

- `GET /api/status`, `GET /api/config`, `GET /probe`
- `POST /api/config`, `POST /api/clock`, `POST /api/reboot`, `POST /api/reconnect`

Flash matching firmware (`esp` + `lite`) so USB `cmp` commands are available.
