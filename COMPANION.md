# Njörðr Seas' CYD miner (Windows)

USB mining control for the ESP32-2432S028. The **PC** talks to a Bitcoin stratum pool; the **board** only SHA-256 hashes work received over USB-C.

## Recommended download (all-in-one)

**[`CYD-Miner-Setup.exe`](flash/downloads/CYD-Miner-Setup.exe)** — Windows setup wizard with:

- Njörðr Seas' CYD miner app
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
2. Open **Njörðr Seas' CYD miner** → select COM → **Update board** (pushes bundled firmware over USB @ **0x0**)  
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
- **0.8.57** — Fetch board FW: spawn off mine-worker (never stuck behind auto-bench); faster raw URLs + timeouts
- **0.8.56** — Bench boards: immediate busy UI + shorter D0 tune window (no silent USB timeout / dead click)
- **0.8.55** — Fix USB connect + update fetch: link immediately, defer auto-bench; run update HTTP off mine-worker thread
- **0.8.54** — Auto-bench each board on USB/Wi‑Fi connect (D0 path pick + NVS lock)
- **0.8.53** — ESP32-D0 build (`*-d0.bin`); Bench auto-times HW/HW+/HW/SW and locks the fastest path in NVS
- **0.8.52** — Neon-blue lightning splashes (Companion storm, logo halo, phone monitor + web UI)
- **0.8.51** — Personal phone QR per Companion install (token-gated :19285); optional remote host for travel; Expo Scan QR
- **0.8.50** — Phone monitor: Companion LAN HTTP API on :19285 (`/api/status` + mobile web UI); Expo iOS/Android app in `mobile/`
- **0.8.49** — CYD LCD: full-screen logo; status strip shows link + hashrate + Wi‑Fi IP only
- **0.8.48** — API feeds pull JSON/text/CSV/files from multiple sources (saves under ApiDownloads\\)
- **0.8.47** — Mine tab Data flow indicator (Pool ↔ Companion ↔ Board; animated jobs/shares)
- **0.8.46** — stabilize Board telemetry (hold rate/hashes/job/nonce across status soft-fails; no blink to zero)
- **0.8.45** — multi-USB: skip hanging PCI COM; serial probe + timed open; require pong; USB cmp answers during SoftAP boot; Event log shows per-port miss/ok
- **0.8.44** — Companion self-update clean-sweeps the install dir (drops stale kit files) before promoting the new build
- **0.8.43** — fix stratum accept/reject counting; hero logo 3× + **Seas'** brand; Bench boards retunes HW path for max H/s
- **0.8.42** — board SoftAP/STA Wi‑Fi + TCP `cmp`; Find workers scans USB + Bluetooth + Wi‑Fi beacons (parallel COM probe)
- **0.8.41** — default stratum `stratum+tcp://btc.hmpool.io:3335` (HM Pool preset)
- **0.8.40** — stabilize hashrate display (hold on missed polls; heavier board EMA; no slow bleed to 0)
- **0.8.39** — multi-board: auto-link every CYD found by scan; Add board while one is connected; do not merge mac=unknown boards
- **0.8.38** — fix `cmp status` USB timeouts while mining (mineB yields on RX; USB/mineB equal priority; longer Companion waits)
- **0.8.37** — fix Find CYD workers: open probes without DTR reset, settle/retry after boot, longer timeouts, background scan + per-port Event log
- **0.8.36** — push board H/s toward ~1020 kH/s (real hashes); rename product to **Njörðr Seas' CYD miner**; skip LCD SPI while mining
- **0.8.35** — brand logo replaces large CYD title; workers identified by board MAC; COM list shows every OS serial port with USB chip details
- **0.8.34** — fix phantom hashrate when USB disconnected; drop dead boards; report real mining flag; OpenUsb no longer wipes multi-board fleet (SHA256d verify scripts PASS)
- **0.8.33** — Njörðr theme (deep blues + lightning); hide Windows console windows for flash/update exes
- **0.8.32** — restore Event log on Mine; Debug/Terminal tab stays hidden
- **0.8.31** — hide Debug/Terminal tab from the Companion UI
- **0.8.30** — fix hashrate dropping to 0 (keep sample window across jobs; Companion holds last rate on empty polls)
- **0.8.29** — after Update board: wait 1s, disconnect, reconnect USB
- **0.8.28** — loading spinner overlay during board update (no terminal spam); live bar °F
- **0.8.27** — Update board erases flash (`erase-flash`) before writing new firmware
- **0.8.26** — lower pool share latency (tight loop, flush, harvest-first) + faster USB (460800, bigger drains)
- **0.8.25** — fix fake 70 MH/s spike (count only real hashes); tighten HW pace; honest ~200–800 kH/s CYD ceiling

## Recommended pool (ESP32-friendly)

| Field | Value |
|-------|--------|
| Stratum | `stratum+tcp://btc.hmpool.io:3335` |
| Worker | Your **Bitcoin** address |
| Password | `x` |

Low share difficulty solo pool (NerdMiner-compatible). Finding a BTC block is a lottery.

Alternates: `public-pool.io:21496`, `pool.nerdminer.io:3333`, `pool.nerdminers.org:3333`

## Tabs

- **Mine** — USB, pool, live hashrate (kH/s), stratum status, event log
- **Settings** — preferences, API feeds, update app/board

## USB protocol

```text
cmp ping / status / config / clock / reboot / stop / bench / wifi
cmp jh <160hex> / jt <64hex> / ja job=&en2=&ntime=
cmp stats accepted=N&rejected=N
board → CMPSHARE nonce=…&job=…&en2=…&ntime=…
```

Same line protocol also runs over **Wi‑Fi TCP :19284**.

## Wi‑Fi workers (0.8.42+)

- Board SoftAP: `Njordr-XXXX` / password `njordrseas` (XXXX from MAC)
- Optional STA: `cmp wifi ssid=…&pass=…` (saved in NVS)
- UDP beacon `CYDBOARD|…` on port **19284**; Companion **Find CYD workers** listens and auto-links
- Join the SoftAP from the PC **or** put the board on your LAN via STA, then scan
