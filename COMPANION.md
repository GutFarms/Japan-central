# CYD Companion (Windows)

GPU **egui** desktop app — control interface for the ESP32-2432S028 miner.
Fetches market/network data on the PC and pushes a ticker to the board over USB.

## Download / build

```bash
./scripts/build-companion-windows.sh
```

Outputs: `dist/CYD-Companion-App-Only.zip`, `Portable.zip`, `Setup.exe`

## First-time setup

1. Flash C++ firmware  
2. Plug USB — board shows **Waiting for app**  
3. Run `cyd-companion.exe` → COM → **Connect**  
4. **Setup** → WiFi + pool → **Save & reboot**

## Network data over USB

While USB is connected, the **Markets** tab refreshes CoinGecko on the PC and sends:

```text
cmp netdata source=coingecko&text=LTC+%2485.2%2B1.2%25+%C2%B7+...
→ CMPACK net
```

The board shows that ticker on the mining screen. No board Wi‑Fi required for prices.

## USB protocol

```text
cmp ping / status / config / set / clock / reboot
cmp netdata source=…&text=…     → CMPACK net
```
