# Flash the ESP32-2432S028 (CYD) scrypt miner

Chip: **ESP32** (WROOM-32) · Board: **ESP32-2432S028** · Image: `esp,lite`

## Save `.bin` to your PC (recommended)

```bash
./scripts/build-flash-images.sh
./scripts/serve-web-flasher.sh
```

Open **http://127.0.0.1:8080/web/** → **Save merged.bin to PC**.

Downloads as:

`esp32-2432s028-scrypt-miner-merged.bin` — flash at **`0x0`** (~1 MiB, bootloader + partitions + app)

Verify (optional):

```bash
cd flash && sha256sum -c SHA256SUMS.txt
```

Or copy without the browser:

```bash
cp flash/esp32-2432s028-scrypt-miner-merged.bin ~/Downloads/
# Windows (Git Bash): cp flash/esp32-2432s028-scrypt-miner-merged.bin "$USERPROFILE/Downloads/"
```

## Flash from the browser

Chrome / Edge only:

1. http://127.0.0.1:8080/web/ (page auto-loads the project image when available)
2. **Save merged.bin to PC** if you want a local copy
3. **Connect & flash** → select the COM / tty port  
   Hold **BOOT** while tapping **RESET** if connect stalls
4. Wait for “Flash complete”

One-click alternate: http://127.0.0.1:8080/web/install.html

## Flash from the CLI

```bash
# Windows
espflash write-bin -p COM6 0x0 flash/esp32-2432s028-scrypt-miner-merged.bin
espflash monitor -p COM6

# Linux / macOS
espflash write-bin -p /dev/ttyUSB0 0x0 flash/esp32-2432s028-scrypt-miner-merged.bin
espflash monitor -p /dev/ttyUSB0

# Or ELF path (auto bootloader)
./scripts/flash-cyd.sh COM6
```

Install **CH340** drivers on Windows if no COM port appears. Serial monitor: **115200**.

## Image files

| File | Address | Notes |
|------|---------|-------|
| `flash/esp32-2432s028-scrypt-miner-merged.bin` | `0x0` | **Use this** — padded only to end of app (~1 MiB) |
| `flash/esp32-2432s028-scrypt-miner.bin` | `0x10000` | App only |
| `flash/SHA256SUMS.txt` | — | Checksums from last build |

## First boot

1. **WiFi scan list** on first setup — tap a network (or **type** / **skip**). Then password (if needed), address / password / stratum, BLE. USB serial (115200) also works: enter scan index, SSID, or `-`
2. If taps do nothing, use serial — it always works in parallel with touch
3. After DHCP, an **ONLINE** screen shows the IP; also on the RADIO tab / MINE URL line
4. Open **`http://<board-ip>/`** on your phone/PC. JSON: `/api/status`

## Not for

- ESP32-S2 / C3 / S3 modules (wrong chip image)
- JCHC-1 ASIC controller (hardware-only package)
