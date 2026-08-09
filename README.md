# ESP32-2432S028 Scrypt Miner (Cheap Yellow Display)

**C++ only.** The board mines Litecoin-style scrypt and shows a basic stats screen.
**CYD Companion** (Windows) is the only control interface — WiFi, pool, clock, reboot.

| Piece | Path |
|-------|------|
| Firmware | `firmware-cpp/` (PlatformIO / Arduino) |
| Control app | `companion/` (egui GPU UI; USB control + network data bridge) |
| Flash images | `flash/` |

## Board (mining only)

- Litecoin scrypt `N=1024` (TMTO, fits classic ESP32 + WiFi)
- LCD: hashrate, pool state, accepts/rejects, CPU MHz, WiFi/IP
- UART0: `cmp` protocol only — no on-device typing / touch setup
- Unconfigured boards show **Waiting for app**

## App (control)

1. Flash merged firmware @ `0x0`
2. Plug USB (CH340)
3. Run `cyd-companion.exe` → COM → **Connect**
4. Fill WiFi + Pool → **Save & reboot**

## Build & flash

```bash
./scripts/build-flash-images.sh      # → flash/*-merged.bin
./scripts/build-companion-windows.sh # → dist/CYD-Companion-*.zip / Setup.exe
./scripts/flash-cyd.sh COM6          # or /dev/ttyUSB0
./scripts/serve-web-flasher.sh       # browser flasher + app downloads
```

Full verify: `./scripts/verify-project.sh`

## Hardware

| Item | Notes |
|------|--------|
| Board | ESP32-2432S028 (CYD) |
| Flash | 4 MB · DIO · 40 MHz |
| Display | ILI9341 320×240 |
| Serial | CH340 → UART0 @ 115200 |

See [FLASH.md](FLASH.md) and [COMPANION.md](COMPANION.md).
