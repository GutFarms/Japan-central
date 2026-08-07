# Flash the ESP32-2432S028 (CYD) scrypt miner

Chip: **ESP32** (WROOM-32) · Board: **ESP32-2432S028** · Image: `esp,lite`

## Quick flash (prebuilt merged image)

From this repo (after building, or using artifacts):

```bash
# Linux / macOS — replace PORT
espflash write-bin -p /dev/ttyUSB0 0x0 flash/esp32-2432s028-scrypt-miner-merged.bin
espflash monitor -p /dev/ttyUSB0

# Windows — replace COMx (Device Manager → Ports)
espflash write-bin -p COM6 0x0 flash/esp32-2432s028-scrypt-miner-merged.bin
espflash monitor -p COM6
```

Or one-shot ELF flash (builds bootloader segments automatically):

```bash
./scripts/flash-cyd.sh COM6
```

Hold **BOOT** while tapping **RESET** if the port does not enter download mode. Use a data-capable USB cable; install **CH340** drivers on Windows if no COM port appears.

Serial monitor baud: **115200**.

## Build flashable images yourself

```bash
. ./export-esp.sh   # or: . $HOME/export-esp.sh
./scripts/build-flash-images.sh
```

Produces:

| File | Flash address | Notes |
|------|---------------|-------|
| `flash/esp32-2432s028-scrypt-miner-merged.bin` | `0x0` | Bootloader + partitions + app (recommended) |
| `flash/esp32-2432s028-scrypt-miner.bin` | `0x10000` | App only (needs bootloader already on device) |
| `target/xtensa-esp32-none-elf/release/esp32-s3-scrypt-miner` | via `espflash flash` | ELF (preferred when building from source) |

## First boot

1. Open serial monitor at 115200.
2. Enter address, password, stratum, WiFi SSID/password, BLE name (`-` to skip BLE).
3. GUI should show on the ILI9341; tabs via short/long **BOOT**.

## Not for

- ESP32-S2 / ESP32-C3 / ESP32-S3 modules (wrong chip image)
- JCHC-1 ASIC controller PCB (hardware-only; no firmware image yet)
