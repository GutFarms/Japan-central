# Japan-central — CYD SHA-256 miner

ESP32-2432S028 boards **hash Bitcoin SHA-256** and, with STA + pool configured, **mine independently** to the stratum pool. **Njörðr Seas' CYD miner** (Windows Companion) monitors H/s, pushes firmware, and can feed USB/Wi‑Fi jobs until a board is indep-authorized.

## Pieces

| Path | Role |
|------|------|
| `firmware-cpp/` | C++ firmware — ESP32 HW SHA-256d + LCD + USB `cmp` + onboard pool |
| `companion/` | egui app — monitor / job bridge / OTA / Setup |
| `flash/` | Merged `.bin` images + Windows downloads |
| `scripts/clean-project.sh` | Wipe local `.pio` / `target` / `dist` caches |

## Flash & mine

1. Prefer in-app **Push update** (USB OTA). Use **Flash (BOOT)** only for blank boards.
2. Power on → Companion **Connect** → BTC address → **Start mining**
3. With home Wi‑Fi, boards take over the pool; Companion shows fleet H/s
4. **Setup** tab or SoftAP phone portal (`http://10.88.88.1/`) to save home Wi‑Fi credentials

Default pool: `stratum+tcp://btc.hmpool.io:3337` · worker = **Bitcoin address** · password `x`

## Build

```bash
./scripts/build-flash-images.sh
./scripts/build-companion-windows.sh
./scripts/verify-project.sh
```

See [`COMPANION.md`](COMPANION.md) for downloads and ownership rules.
