# CYD scrypt miner (C++ / USB hash)

ESP32-2432S028 firmware: **scrypt hashing only**. No Wi‑Fi. Pool traffic stays on the PC companion; work and shares move over USB-C (`cmp`).

## Build

```bash
./scripts/build-flash-images.sh
```

Flash `flash/esp32-2432s028-scrypt-miner-merged.bin` at **0x0** (DIO, 4 MB, 40 MHz).

## Runtime

- LCD: waiting for USB → hashrate / accept-reject (from PC) / USB link
- UART0 @ 115200: companion protocol only
- Wi‑Fi radio forced **OFF** at boot

## Protocol (board)

See [`../COMPANION.md`](../COMPANION.md).
