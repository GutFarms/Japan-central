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

## Features

| Tab | What it does |
|-----|----------------|
| Dashboard | Live hashrate, pool, WiFi, CPU MHz |
| Settings | WiFi / stratum / worker / password → flash |
| Overclock | CPU 80 / 160 / 240 MHz (soft-reset apply) |
| Discover | Probe LAN `/probe` for SCRYPT-CYD boards |

Auth for writes = **pool password**.

## Board APIs used

- `GET /api/status`, `GET /api/config`, `GET /probe`
- `POST /api/config`, `POST /api/clock`, `POST /api/reboot`, `POST /api/reconnect`
