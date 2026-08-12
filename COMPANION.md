# CYD Companion (Windows)

USB mining control for the ESP32-2432S028. The **PC** talks to a Bitcoin stratum pool; the **board** only SHA-256 hashes work received over USB-C.

## Recommended download (all-in-one)

**`CYD-Miner-Setup.exe`** — Windows setup wizard with:

- CYD Companion app
- Firmware `esp32-2432s028-sha256-miner-merged.bin`
- `Flash-Firmware.bat` helper + flash docs
- Start Menu / Desktop shortcuts

Also available:

| Package | Contents |
|---------|----------|
| `CYD-Miner-Portable.zip` | Same kit, no installer |
| `CYD-Companion-App-Only.zip` | Just `cyd-companion.exe` |
| `CYD-Companion-Setup.exe` | Alias of the full miner setup |

## Build

```bash
./scripts/build-flash-images.sh
./scripts/build-companion-windows.sh
```

## Use

1. Run **CYD-Miner-Setup.exe** (or unpack the portable kit)
2. Open **CYD Companion** → select COM → **Update board** (pushes bundled firmware over USB @ **0x0**)  
   — or run `Flash-Firmware.bat` once if you prefer
3. **Connect** USB → enter stratum / **Bitcoin address** / password → **Start mining**

`Update board` stops mining, frees the COM port, flashes `Firmware\esp32-2432s028-sha256-miner-merged.bin` with bundled `Tools\espflash.exe`, then reconnects. If flash fails: hold **BOOT**, tap **RESET**, release **BOOT**, then retry.

## Companion extras (0.8.5+)
- First-run setup wizard
- Version-aware **Update board** + **Fetch latest FW**
- Pool auto-reconnect, share dedupe, accept/reject latency
- Session luck (expected vs accepted) + SHA path (HW / HW+ / HW/SW)
- LCD ticker, pool presets, COM auto-connect, copy logs
- **Find CYD workers** — USB scan for companion firmwares + LAN Companion peer discovery; connect multiple USB boards and fan-out jobs
- Custom **API feeds** (Settings) to pull external HTTPS data into the app

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
cmp jh <160hex> / jt <64hex> / ja job=&en2=&ntime=
cmp stats accepted=N&rejected=N
board → CMPSHARE nonce=…&job=…&en2=…&ntime=…
```
