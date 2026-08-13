# CYD Companion (Windows)

USB mining control for the ESP32-2432S028. The **PC** talks to a Bitcoin stratum pool; the **board** only SHA-256 hashes work received over USB-C.

## Recommended download (all-in-one)

**[`CYD-Miner-Setup.exe`](flash/downloads/CYD-Miner-Setup.exe)** — Windows setup wizard with:

- CYD Companion app
- Firmware `esp32-2432s028-sha256-miner-merged.bin`
- `Flash-Firmware.bat` helper + flash docs
- Start Menu / Desktop shortcuts

Also available:

| Package | Contents |
|---------|----------|
| [`cyd-companion.exe`](flash/downloads/cyd-companion.exe) | App binary (Update board auto-fetches Firmware + espflash) |
| `CYD-Miner-Portable.zip` | Same kit, no installer |
| `CYD-Companion-App-Only.zip` | App + `Firmware\` + `Tools\espflash.exe` |
| `CYD-Companion-Setup.exe` | Alias of the full miner setup |

See [`flash/downloads/README.md`](flash/downloads/README.md) for checksums.

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

In **Settings**:
- **Update app** — download the latest Companion Windows build and restart
- **Fetch latest FW** — pull the newest board `merged.bin` into `Firmware\`
- **Update board** — flash that image to the CYD over USB

## Companion extras (0.8.5+)
- First-run setup wizard
- Version-aware **Update board** + **Fetch latest FW**
- Pool auto-reconnect, share dedupe, accept/reject latency
- Session luck (expected vs accepted) + SHA path (HW / HW+ / HW/SW)
- LCD ticker, pool presets, COM auto-connect, copy logs
- **Find CYD workers** — USB scan for companion firmwares + LAN Companion peer discovery; connect multiple USB boards and fan-out jobs
- Custom **API feeds** (Settings) to pull external HTTPS data into the app
- Stable board hashrate (EMA + longer sample window; no zero-on-job flicker)
- **Hash-focus firmware** — strip LCD animations/ticker, one-time HW calibrate, dedicated core-0 mine task, lean USB
- **0.8.17** — fix start/stop reboot loop (mine task must `vTaskDelay` so the watchdog idle task can run)
- **0.8.18** — Update board auto-downloads firmware + espflash when missing; waits for COM release; retries 115200
- **0.8.21** — Check/Update Companion app + Fetch latest board FW from repo downloads
- **0.8.22** — fix Windows self-update bat (retry while `.new` remains; do not relaunch a locked old exe)
- **0.8.23** — fetch updates via GitHub API / commit-pinned raw (branch raw CDN can serve stale VERSION/bins)
- **0.8.24** — fix Update board: espflash reject `ESPFLASH_SKIP_UPDATE_CHECK=1` (needs true/false); skip Windows Store python stubs
- **0.8.29** — after Update board: wait 1s, disconnect, reconnect USB
- **0.8.28** — loading spinner overlay during board update (no terminal spam); live bar °F
- **0.8.27** — Update board erases flash (`erase-flash`) before writing new firmware
- **0.8.26** — lower pool share latency (tight loop, flush, harvest-first) + faster USB (460800, bigger drains)
- **0.8.25** — fix fake 70 MH/s spike (count only real hashes); tighten HW pace; honest ~200–800 kH/s CYD ceiling

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
