# CYD Companion (egui)

GPU desktop control app for the ESP32-2432S028 miner — restored bubbly blue/lime UI.

## Roles

- **Control** the board over USB (`cmp` protocol): setup, WiFi, pool, clock
- **Bridge network data**: fetch CoinGecko (and related) on the PC, push a ticker to the board with `cmp netdata`

The board does not need internet for market prices when the companion is connected.

## Build

```bash
./scripts/build-companion-windows.sh
# or: cd companion && cargo build --release --target x86_64-pc-windows-gnu
```
