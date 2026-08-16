# Njörðr Seas' CYD miner (Windows)

USB / Wi‑Fi mining control for the ESP32-2432S028. **Boards mine independently** to the Bitcoin pool when STA + pool are configured. Companion monitors H/s, pushes firmware, and feeds USB jobs only while a board is not yet indep-authorized.

## Recommended download

**[`CYD-Miner-Setup.exe`](flash/downloads/CYD-Miner-Setup.exe)** — Windows setup wizard with app, firmware, and flash helpers.

Also: `cyd-companion.exe`, `CYD-Miner-Portable.zip`, `CYD-Companion-App-Only.zip` (see [`flash/downloads/README.md`](flash/downloads/README.md)).

## Build

```bash
./scripts/build-flash-images.sh
./scripts/build-companion-windows.sh
./scripts/verify-project.sh
```

## Use

1. Run **CYD-Miner-Setup.exe** (or unpack the portable kit)
2. Open Companion → select COM → **Update board**
   - **Push update** — USB app OTA (no BOOT) for linked boards
   - **Push update (Wi‑Fi)** — TCP OTA when a Wi‑Fi worker is linked
   - **Flash (BOOT)** — blank / download-mode boards only (hold BOOT → RESET → Ready)
3. **Connect** → enter stratum / **Bitcoin address** → **Start mining**
4. With home Wi‑Fi + pool URL, boards authorize onboard and Companion stops job push (monitor mode)

Default pool: `stratum+tcp://btc.hmpool.io:3337` · worker = **address.companion** · password `x`

## SoftAP setup

Board SoftAP is **`10.88.88.1`** (open `Njordr-XXXX`). Join from phone → captive portal / Companion Setup to push home Wi‑Fi. Prefer STA address `192.168.1.88` when the LAN DHCP pool allows it — that is **not** SoftAP.

## Ownership (no fighting doubles)

| Concern | Primary | Fallback |
|---------|---------|----------|
| Mining jobs | Board `PoolStratum` when STA+pool live | Companion USB/Wi‑Fi jobs while `!mine_indep` |
| Firmware update | Push app OTA | Flash (BOOT) for blank boards |
| Wi‑Fi credentials | Companion **Setup** tab (`cmp wifi` over USB/SoftAP) | Phone SoftAP portal `http://10.88.88.1/` |
| Shares | Board→pool (indep) or Companion harvest (fed) | — |
