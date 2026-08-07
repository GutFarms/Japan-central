# ESP32-S3 Scrypt Miner

Bare-metal Rust firmware that mines **scrypt** proof-of-work on an **ESP32-S3** and shows a multi-screen **GUI** on the onboard LCD. Targets the **LilyGO T-Display-S3** (ST7789 320×170, 8-bit parallel). Uses the board’s **onboard WiFi** (STA + DHCP) and **Bluetooth LE** advertising.

## What it does

- Runs Litecoin-style scrypt: `N=1024` (`2^10`), `r=1`, `p=1`, 32-byte digest
- Reuses ROMix buffers across hashes (≈128 KiB working set)
- **After boot**, prompts over USB serial for **address**, **password**, **stratum**, optional **WiFi**, and **BLE name**
- Starts **WiFi STA + DHCP** when an SSID is set; **BLE** is opt-in (`ble_name`, or `-` to skip) so stratum mining can keep more RAM
- **Stratum TCP client** over WiFi: subscribe, authorize, receive jobs, submit shares (reconnect backoff, share queue, live status)
- **On-device GUI**: splash, setup, mining dashboard, config tab, radio/pool tab, menu
- Saves credentials to flash and auto-loads them on later boots
- Host CLI (`host-miner`), **desktop GUI** (`host-gui`), and unit tests

This is an educational / demo miner. An ESP32-S3 will only manage a few hashes per second at full Litecoin parameters — far below profitable network mining.

## On-device GUI

| Control | Action |
|---------|--------|
| **BOOT** (GPIO0) | Next tab, or move menu highlight |
| **Custom btn** (GPIO14) | Open menu / activate selected item |
| Serial `change` | Password-gated credential edit |
| Serial `radio` / `wifi` / `ble` / `stratum` | Print live radio + pool status |

Tabs: **MINE** · **CONF** · **RADIO** (WiFi/IP/BLE + stratum) · **MENU**.

When WiFi is configured, the firmware connects to `stratum` (`host:port` or `stratum+tcp://…`), runs `mining.subscribe` / `mining.authorize`, mines `mining.notify` jobs with the pool difficulty, and submits shares with `mining.submit`. Without WiFi it falls back to local demo mining.

## Post-boot credentials (saved to flash)

On **first boot**, open the serial monitor and enter:

1. `address` — wallet address or worker name  
2. `password` — pool password (often `x`)  
3. `stratum` — pool location, e.g. `stratum.example.com:3333`  
4. `wifi_ssid` — AP name, or `-` / `skip` to disable WiFi  
5. `wifi_password` — PSK (empty = open network)  
6. `ble_name` — advertised name, or `-` / empty to **skip BLE** (recommended when mining over WiFi)

Values are **written to flash** and **auto-loaded on every later boot**. Legacy v1 (pool-only) blobs still load; WiFi/BLE stay off until you set them via `change`.

### Change credentials (password required)

- Menu → **Change credentials**, or type `change` on serial  
- Or hold **BOOT** at power-on  

3 attempts; failed auth keeps the previous saved config. Password entry echoes `*`.  
Stratum worker/endpoint/password changes **reconnect without reboot**. After changing WiFi/BLE settings, **reboot** so the radio stack picks up the new values.

## Hardware

| Item | Notes |
|------|--------|
| Board | LilyGO T-Display-S3 (ESP32-S3) |
| Display | ST7789, 320×170, parallel bus |
| Buttons | BOOT=GPIO0, custom=GPIO14 |
| Radio | Onboard WiFi + Bluetooth (`esp-radio` + `embassy-net` + `trouble-host` 0.6) |
| RAM | ~64 KiB reclaimed + ~200 KiB heap for radio + scrypt ROMix |

Pin map matches LilyGO’s T-Display-S3 parallel wiring (GPIO5–9, 14–15, 38–42, 45–48).

## Build & flash (device)

Prefer **`esp,lite`** (`N=64`) on device — full Litecoin `N=1024` plus WiFi is tight on RAM and only a few H/s anyway:

```bash
# Install toolchain once: https://docs.esp-rs.org/book/installation/
. $HOME/export-esp.sh   # path printed by espup

cargo +esp run -Zbuild-std=core,alloc --release \
  --target xtensa-esp32s3-none-elf --features esp,lite
```

Full params (more RAM): `--features esp`

The `esp,lite` firmware **links successfully** with the espup Xtensa toolchain (`.cargo/config.toml` includes `-Tlinkall.x`).

## Host demo, GUI & tests

```bash
cargo test --no-default-features
cargo run --no-default-features --features host --bin host-miner --release
cargo run --no-default-features --features host-gui --bin host-gui --release
```

Host CLI also accepts `--wifi-ssid`, `--wifi-password`, and `--ble-name`.

Demo difficulty is set by `DEMO_ZERO_NIBBLES` in `src/bin/main.rs` (default `4`).
