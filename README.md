# Japan-central — CYD USB SHA-256 miner

ESP32-2432S028 board **hashes Bitcoin SHA-256 only**. **Njörðr seas CYD miner** on Windows owns the pool connection; work and shares move over **USB-C**. No board Wi‑Fi.

## Pieces

| Path | Role |
|------|------|
| `firmware-cpp/` | C++ firmware — ESP32 HW SHA-256d + LCD + USB `cmp` |
| `companion/` | egui app — Bitcoin stratum on PC, USB job/share bridge |
| `flash/` | Merged `.bin` images |

## Flash & mine

1. Flash `flash/esp32-2432s028-sha256-miner-merged.bin` @ **0x0** (DIO, 4 MB, 40 MHz)
2. Power on — LCD: **Waiting for USB**
3. Run Companion → COM → **Connect** → BTC address → **Start mining**

Default pool: `stratum+tcp://btc.hmpool.io:3335` · worker = **Bitcoin address** · password `x`

Solo lottery mining for tiny ESP32 hashrate (hardware SHA-256d on classic ESP32 — expect high **kH/s**; software midstate fallback if HW is unavailable).

## Build

```bash
./scripts/build-flash-images.sh
./scripts/build-companion-windows.sh
```

## Protocol

See [`COMPANION.md`](COMPANION.md).
