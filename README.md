# Japan-central — CYD USB SHA-256 miner

ESP32-2432S028 board **hashes Bitcoin SHA-256 only**. **Njörðr Seas' CYD miner** on Windows owns the pool connection; work and shares move over **USB** (data serial / typical USB‑C on the CYD) or SoftAP/STA Wi‑Fi. Nearby boards can join an **ESP-NOW connectivity mesh** through a USB-linked root (power-only leaves OK).

## Pieces

| Path | Role |
|------|------|
| `firmware-cpp/` | C++ firmware — ESP32 HW SHA-256d + LCD + USB `cmp` + mesh bridge |
| `companion/` | egui app — Bitcoin stratum on PC, USB/mesh job/share bridge |
| `flash/` | Merged `.bin` images + Windows downloads |
| `scripts/clean-project.sh` | Wipe local `.pio` / `target` / `dist` caches |

## Flash & mine

1. Flash `flash/esp32-2432s028-sha256-miner-merged.bin` @ **0x0** (DIO, 4 MB, 40 MHz)
2. Power on — LCD: **Waiting for USB**
3. Run Companion → COM → **Connect** → BTC address → **Start mining**
4. Optional: power extra CYDs nearby — they mesh to the USB root (root keeps hashing, throttled)

Default pool: `stratum+tcp://btc.hmpool.io:3337` · worker = **Bitcoin address** · password `x`

## Build

```bash
./scripts/build-flash-images.sh
./scripts/build-companion-windows.sh
./scripts/verify-project.sh
```

Clean local caches (keeps `flash/downloads/`):

```bash
./scripts/clean-project.sh
```

## Protocol

See [`COMPANION.md`](COMPANION.md).
