# ESP32-2432S028 Scrypt Miner (Cheap Yellow Display)

Bare-metal Rust firmware that mines **scrypt** proof-of-work on an **ESP32-2432S028** (CYD) and shows a multi-screen **GUI** on the onboard **ILI9341** TFT. Uses onboard **WiFi** (STA + DHCP) and optional **Bluetooth LE** advertising.

## What it does

- Runs Litecoin-style scrypt: prefer **`N=64` (`lite`)** on this board; full `N=1024` is too RAM-heavy with WiFi
- Reuses ROMix buffers across hashes
- **After boot**, prompts (touch keyboard or USB serial) for **WiFi first**, then **address** / **password** / **stratum**, then optional **BLE**
- Starts **WiFi STA + DHCP** when an SSID is set; **BLE** is opt-in (`ble_name`, or `-` to skip)
- **Stratum TCP client** over WiFi: subscribe, authorize, receive jobs, submit shares
- **LAN web UI** at `http://<board-ip>/` after DHCP (status dashboard + JSON)
- **On-device GUI**: splash, setup, mining dashboard, config tab, radio/pool tab, menu
- Saves credentials to flash and auto-loads them on later boots
- Host CLI (`host-miner`), **desktop GUI** (`host-gui`), and unit tests

Educational / demo miner only — not profitable network mining.

## On-device GUI

| Control | Action |
|---------|--------|
| **Touch** tab strip | Jump to MINE / CONF / RADIO / MENU |
| **Touch** keyboard | Type credentials during setup / auth (OK / skip) |
| **Touch** menu row | Activate option |
| **BOOT** short press | Next tab, or move menu highlight |
| **BOOT** long press (~0.7s) | Open menu / activate selected item |
| Serial `change` | Password-gated credential edit |
| Serial `radio` / `wifi` / `ble` / `stratum` | Print live radio + pool status |

Tabs: **MINE** · **CONF** · **RADIO** · **MENU**.

When WiFi is configured, the firmware connects to `stratum` (`host:port` or `stratum+tcp://…`), runs `mining.subscribe` / `mining.authorize`, mines `mining.notify` jobs with the pool difficulty, and submits shares with `mining.submit`. Without WiFi it falls back to local demo mining.

## Post-boot credentials (saved to flash)

On **first boot**, use the **touch screen** or serial monitor (115200):

1. `wifi_ssid` — **scan & tap** a network (or **type** / **skip** / `-`)  
2. `wifi_password` — PSK (skipped for open networks / WiFi off)  
3. `address` — wallet address or worker name  
4. `password` — pool password (often `x`)  
5. `stratum` — pool location, e.g. `stratum.example.com:3333`  
6. `ble_name` — advertised name, or `-` / empty to **skip BLE** (recommended; WiFi mining skips BLE anyway)

After WiFi + DHCP, the board shows an **ONLINE** screen with the **IP address** and `http://IP/` (also on the RADIO tab).

### Change credentials (password required)

- Menu → **Change credentials**, or type `change` on serial  
- Or hold **BOOT** at power-on  

Stratum worker/endpoint/password changes **reconnect without reboot**. After changing WiFi/BLE settings, **reboot**.

## Hardware

| Item | Notes |
|------|--------|
| Board | **ESP32-2432S028** (Cheap Yellow Display) |
| MCU | ESP32-WROOM-32 |
| Display | ILI9341 SPI, 320×240 landscape, backlight GPIO21 |
| TFT SPI | SCLK=14, MOSI=13, MISO=12, CS=15, DC=2 |
| Buttons | BOOT=GPIO0 (short/long); **touch** tabs + on-screen keyboard |
| Touch | XPT2046 CLK=25 MOSI=32 MISO=39 CS=33 IRQ=36 |
| Serial | USB-UART CH340 → UART0 (TX=1, RX=3) |
| Radio | Onboard WiFi + BLE (`esp-radio` + `embassy-net` + `trouble-host` 0.6) |
| Flash config | Sector at `0x3FF000` (end of 4 MiB window) |

## Build & flash (device)

**See [FLASH.md](FLASH.md).**

```bash
. ./export-esp.sh
./scripts/build-flash-images.sh
./scripts/serve-web-flasher.sh
# → http://127.0.0.1:8080/web/  (Chrome / Edge)
#    Save merged.bin to PC  →  Connect & flash
```

CLI: `./scripts/flash-cyd.sh COM6` or  
`espflash write-bin -p COM6 0x0 flash/esp32-2432s028-scrypt-miner-merged.bin`

Hold **BOOT** + **RESET** if connect stalls; install CH340 drivers on Windows if needed.  
Full scrypt `N=1024` without `lite` is not recommended on this board with WiFi.

## Host demo, GUI & tests

```bash
cargo test --no-default-features
cargo run --no-default-features --features host --bin host-miner --release
cargo run --no-default-features --features host-gui --bin host-gui --release
```

Demo difficulty is set by `DEMO_ZERO_NIBBLES` in `src/bin/main.rs` (default `4`).
