# Japan-central — CYD USB SHA-256 miner

ESP32-2432S028 board **hashes Bitcoin SHA-256 only**. **CYD Companion** on Windows owns the pool connection; work and shares move over **USB-C**. No board Wi‑Fi.

## Pieces

| Path | Role |
|------|------|
| `firmware-cpp/` | C++ firmware — midstate SHA-256d + LCD + USB `cmp` |
| `companion/` | egui app — Bitcoin stratum on PC, USB job/share bridge |
| `flash/` | Merged `.bin` images |

## Flash & mine

1. Flash `flash/esp32-2432s028-sha256-miner-merged.bin` @ **0x0** (DIO, 4 MB, 40 MHz)
2. Power on — LCD: **Waiting for USB**
3. Run Companion → COM → **Connect** → BTC address → **Start mining**

Default pool: `stratum+tcp://public-pool.io:21496` · worker = **Bitcoin address** · password `x`

Solo lottery mining for tiny ESP32 hashrate (custom midstate path — expect hundreds of **kH/s**).

## Build

```bash
./scripts/build-flash-images.sh
./scripts/build-companion-windows.sh
```

## Protocol

See [`COMPANION.md`](COMPANION.md).
