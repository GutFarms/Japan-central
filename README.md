# ESP32-S3 Scrypt Miner

Bare-metal Rust firmware that mines **scrypt** proof-of-work on an **ESP32-S3** and shows live stats on the onboard LCD. Targets the **LilyGO T-Display-S3** (ST7789 320×170, 8-bit parallel).

## What it does

- Runs scrypt with **`N=64`** by default (`2^6`, `r=1`, `p=1`, 32-byte digest)
- Optional `litecoin` feature for `N=1024` (Litecoin-compatible)
- Reuses ROMix buffers across hashes (≈8 KiB at N=64; ≈128 KiB at N=1024)
- Paints hashrate, nonce, shares, and best hash on the T-Display-S3
- Includes a host CLI (`host-miner`) and unit tests for the same miner core

This is an educational / demo miner — not profitable network mining.

## Hardware

| Item | Notes |
|------|--------|
| Board | LilyGO T-Display-S3 (ESP32-S3) |
| Display | ST7789, 320×170, parallel bus |
| RAM | Firmware allocates a ~192 KiB heap for scrypt ROMix |

Pin map matches LilyGO’s T-Display-S3 parallel wiring (GPIO5–9, 15, 38–42, 45–48).

## Build & flash (device)

Install the Espressif Rust toolchain and `espflash`, then:

```bash
cargo +esp build -Zbuild-std=core,alloc --release \
  --target xtensa-esp32s3-none-elf --features esp
cargo +esp run -Zbuild-std=core,alloc --release \
  --target xtensa-esp32s3-none-elf --features esp
```

Litecoin-compatible params (`N=1024`):

```bash
cargo +esp run -Zbuild-std=core,alloc --release \
  --target xtensa-esp32s3-none-elf --features esp,litecoin
```

## Host demo & tests

No ESP toolchain required:

```bash
cargo test --no-default-features
cargo run --no-default-features --features host --bin host-miner --release
# optional difficulty (leading zero nibbles), default 4:
cargo run --no-default-features --features host --bin host-miner --release -- 5
```

## On-screen UI

- **SCRYPT** brand + parameter line
- Hashrate (H/s), mining status
- Current nonce, share count, best hash prefix

Demo difficulty is set by `DEMO_ZERO_NIBBLES` in `src/bin/main.rs` (default `4`).
