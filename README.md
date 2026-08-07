# ESP32-S3 Scrypt Miner

Bare-metal Rust firmware that mines **scrypt** proof-of-work on an **ESP32-S3** and shows a multi-screen **GUI** on the onboard LCD. Targets the **LilyGO T-Display-S3** (ST7789 320×170, 8-bit parallel). Uses the board’s **onboard WiFi** (STA + DHCP) and **Bluetooth LE** advertising.

## What it does

- Runs Litecoin-style scrypt: `N=1024` (`2^10`), `r=1`, `p=1`, 32-byte digest
- Reuses ROMix buffers across hashes (≈128 KiB working set)
- **After boot**, prompts over USB serial for **address**, **password**, **stratum**, optional **WiFi**, and **BLE name**
- Starts **WiFi STA + DHCP** when an SSID is set; always advertises **BLE** (connectable GATT)
- **On-device GUI**: splash, setup, mining dashboard, config tab, radio tab, menu
- Saves credentials to flash and auto-loads them on later boots
- Host CLI (`host-miner`), **desktop GUI** (`host-gui`), and unit tests

This is an educational / demo miner. An ESP32-S3 will only manage a few hashes per second at full Litecoin parameters — far below profitable network mining.

## On-device GUI

| Control | Action |
|---------|--------|
| **BOOT** (GPIO0) | Next tab, or move menu highlight |
| **Custom btn** (GPIO14) | Open menu / activate selected item |
| Serial `change` | Password-gated credential edit |
| Serial `radio` / `wifi` / `ble` | Print live radio status |

Tabs: **MINE** · **CONF** · **RADIO** (WiFi phase/IP + BLE) · **MENU**.

## Post-boot credentials (saved to flash)

On **first boot**, open the serial monitor and enter:

1. `address` — wallet address or worker name  
2. `password` — pool password (often `x`)  
3. `stratum` — pool location, e.g. `stratum.example.com:3333`  
4. `wifi_ssid` — AP name, or `-` / `skip` to disable WiFi  
5. `wifi_password` — PSK (empty = open network)  
6. `ble_name` — advertised name (empty defaults to `SCRYPT`)

Values are **written to flash** and **auto-loaded on every later boot**. Legacy v1 (pool-only) blobs still load; WiFi stays off and BLE name defaults to `SCRYPT` until you run `change`.

### Change credentials (password required)

- Menu → **Change credentials**, or type `change` on serial  
- Or hold **BOOT** at power-on  

3 attempts; failed auth keeps the previous saved config. Password entry echoes `*`.  
After changing WiFi/BLE settings, **reboot** so the radio stack picks up the new values.

## Hardware

| Item | Notes |
|------|--------|
| Board | LilyGO T-Display-S3 (ESP32-S3) |
| Display | ST7789, 320×170, parallel bus |
| Buttons | BOOT=GPIO0, custom=GPIO14 |
| Radio | Onboard WiFi + Bluetooth (esp-radio + embassy-net + trouble-host) |
| RAM | ~64 KiB reclaimed + ~200 KiB heap for radio + scrypt ROMix |

Pin map matches LilyGO’s T-Display-S3 parallel wiring (GPIO5–9, 14–15, 38–42, 45–48).

## Build & flash (device)

```bash
cargo +esp run -Zbuild-std=core,alloc --release \
  --target xtensa-esp32s3-none-elf --features esp
```

Faster demo / less RAM (`N=64`): `--features esp,lite`

## Host demo, GUI & tests

```bash
cargo test --no-default-features
cargo run --no-default-features --features host --bin host-miner --release
cargo run --no-default-features --features host-gui --bin host-gui --release
```

Host CLI also accepts `--wifi-ssid`, `--wifi-password`, and `--ble-name`.

Demo difficulty is set by `DEMO_ZERO_NIBBLES` in `src/bin/main.rs` (default `4`).
