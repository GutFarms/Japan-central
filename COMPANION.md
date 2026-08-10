# CYD Companion (Windows)

USB mining control for the ESP32-2432S028. The **PC** talks to a Bitcoin stratum pool; the **board** only SHA-256 hashes work received over USB-C.

## Download / build

```bash
./scripts/build-companion-windows.sh
```

Outputs: `dist/CYD-Companion-App-Only.zip`, `Portable.zip`, `Setup.exe`

## Use

1. Flash C++ firmware (`esp32-2432s028-sha256-miner-merged.bin` @ **0x0**)
2. Plug USB-C — board shows **Waiting for USB**
3. Run `cyd-companion.exe` → COM → **Connect**
4. Enter stratum / **Bitcoin address** / password → **Start mining**

## Recommended pool (ESP32-friendly)

| Field | Value |
|-------|--------|
| Stratum | `stratum+tcp://public-pool.io:21496` |
| Worker | Your **Bitcoin** address |
| Password | `x` |

Low share difficulty solo pool (NerdMiner-compatible). Finding a BTC block is a lottery.

Alternates: `pool.nerdminer.io:3333`, `pool.nerdminers.org:3333`

## Tabs

- **Mine** — USB, pool, live hashrate (kH/s), stratum TX/RX, logs
- **Debug / Terminal** — raw `cmp` commands to the board

## USB protocol

```text
cmp ping / status / config / clock / reboot / stop / bench
cmp job header=<160hex>&target=<64hex>&job=…&en2=…&ntime=…
cmp stats accepted=N&rejected=N
board → CMPSHARE nonce=…&job=…&en2=…&ntime=…
```
