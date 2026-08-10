# Japan-central — CYD USB scrypt miner

ESP32-2432S028 board **hashes only**. **CYD Companion** on Windows owns the pool connection; work and shares move over **USB-C**. No board Wi‑Fi.

## Pieces

| Path | Role |
|------|------|
| `firmware-cpp/` | C++ firmware — scrypt + LCD + USB `cmp` |
| `companion/` | egui app — stratum on PC, USB job/share bridge |
| `flash/` | Merged `.bin` images |

## Flash & mine

1. Flash `flash/esp32-2432s028-scrypt-miner-merged.bin` @ **0x0** (DIO, 4 MB, 40 MHz)
2. Power on (not BOOT) — LCD: **Waiting for USB**
3. Run Companion → COM → **Connect** → pool fields → **Start mining**

Default pool: `stratum+tcp://scrypt.mysolopool.com:3341` · worker = Litecoin address · password `d=1`

## Build

```bash
./scripts/build-flash-images.sh
./scripts/build-companion-windows.sh
```

## Protocol

See [`COMPANION.md`](COMPANION.md).
