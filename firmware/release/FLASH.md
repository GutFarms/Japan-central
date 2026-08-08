# Flash binaries

Prebuilt images for the ESP32-CYD PC/GPU monitor.

| File | Use |
| --- | --- |
| `esp32-cyd-pc-monitor-merged.bin` | **Recommended** — full image (bootloader + partitions + app). Flash at `0x0`. |
| `esp32-cyd-pc-monitor.bin` | App only. Flash at `0x10000` (needs bootloader + partitions already present). |
| `bootloader.bin` | Bootloader at `0x1000` |
| `partitions.bin` | Partition table at `0x8000` |

## One-shot flash (merged)

```bash
esptool.py --chip esp32 --port COMx --baud 921600 write_flash 0x0 esp32-cyd-pc-monitor-merged.bin
```

Linux/macOS example:

```bash
esptool.py --chip esp32 --port /dev/ttyUSB0 --baud 921600 write_flash 0x0 esp32-cyd-pc-monitor-merged.bin
```

## Separate images

```bash
esptool.py --chip esp32 --port /dev/ttyUSB0 --baud 921600 write_flash \
  0x1000 bootloader.bin \
  0x8000 partitions.bin \
  0x10000 esp32-cyd-pc-monitor.bin
```

Or with PlatformIO from the `firmware/` folder:

```bash
pio run -t upload
```

These prebuilt images are compiled with the Wi‑Fi credentials in `include/secrets.h` (SSID: `Stargate Command`).

After flashing, leave the board plugged into USB and run the host agent over serial:

```bash
python host/agent.py --serial auto
```

Wi‑Fi/UDP remains available as an optional second path.
