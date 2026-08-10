# CYD Companion (Windows)

USB mining control for the ESP32-2432S028. The **PC** talks to the pool; the **board** only hashes work received over USB-C (no board Wi‑Fi).

## Download / build

```bash
./scripts/build-companion-windows.sh
```

Outputs: `dist/CYD-Companion-App-Only.zip`, `Portable.zip`, `Setup.exe`

## Use

1. Flash C++ firmware (`esp32-2432s028-scrypt-miner-merged.bin` @ **0x0**)
2. Plug USB-C — board shows **Waiting for USB**
3. Run `cyd-companion.exe` → select COM → **Connect**
4. Enter stratum / worker / password → **Start mining**

## Recommended pool

| Field | Value |
|-------|--------|
| Stratum | `stratum+tcp://scrypt.mysolopool.com:3341` |
| Worker | Your **Litecoin** address |
| Password | `d=1` |

LTC + DOGE merged mining at low share difficulty.

## USB protocol

```text
cmp ping / status / config / clock / reboot / stop
cmp job header=<160hex>&target=<64hex>&job=…&en2=…&ntime=…
cmp stats accepted=N&rejected=N
cmp netdata source=…&text=…          (optional LCD ticker)
board → CMPSHARE nonce=…&job=…&en2=…&ntime=…
```
