# Flashing the CYD (ESP32-2432S028)

## Preferred image

`flash/esp32-2432s028-scrypt-miner-merged.bin` at address **0x0**

| Setting | Value |
|---------|--------|
| Chip | ESP32 |
| Flash mode | DIO |
| Flash size | 4 MB |
| Flash freq | 40 MHz |

## Build

```bash
./scripts/build-flash-images.sh
```

## Flash

```bash
./scripts/flash-cyd.sh COM6
# or browser: ./scripts/serve-web-flasher.sh → http://127.0.0.1:8080/web/
```

Hold **BOOT** + **RESET** if the port will not enter download mode. Install CH340 drivers on Windows if needed.

After flash, power on **without** holding BOOT. Configure with **CYD Companion** over USB.
