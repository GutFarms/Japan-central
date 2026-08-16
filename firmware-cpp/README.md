# CYD SHA-256 miner (C++ / USB hash)

ESP32-2432S028 firmware: **Bitcoin double-SHA256** with a custom midstate compressor (IRAM hot path). SoftAP/STA Wi‑Fi for Setup and onboard pool mining. Work and shares move over USB `cmp` or Wi‑Fi TCP while Companion-fed; boards mine independently once STA + pool are configured.

## Build

```bash
./scripts/build-flash-images.sh
```

Flash `flash/esp32-2432s028-sha256-miner-merged.bin` at **0x0** (DIO, 4 MB, 40 MHz).

## Runtime

- Dual-core midstate SHA-256d
- LCD: kH/s, accepts/rejects, USB link
- UART0 @ 115200: companion protocol only

## Protocol

See [`../COMPANION.md`](../COMPANION.md).
