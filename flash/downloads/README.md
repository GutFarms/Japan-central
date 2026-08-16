# Njörðr Seas' CYD miner downloads (`0.8.187`)

Primary firmware is the **ESP32-D0** auto-tune build (`esp32-2432s028-sha256-miner-d0-merged.bin`, also the canonical `esp32-2432s028-sha256-miner-merged.bin`). Each board auto-benches on connect; **Bench boards (D0)** re-runs the path pick anytime.

| File | What it is |
|------|------------|
| **[CYD-Miner-Setup.exe](./CYD-Miner-Setup.exe)** | Installer — Companion + firmware (recommended) |
| **[cyd-companion.exe](./cyd-companion.exe)** | Standalone app — double-click to run |
| [CYD-Companion-App-Only.zip](./CYD-Companion-App-Only.zip) | App zip |
| [CYD-Miner-Portable.zip](./CYD-Miner-Portable.zip) | Full portable kit |
| [esp32-2432s028-sha256-miner-d0-merged.bin](./esp32-2432s028-sha256-miner-d0-merged.bin) | D0 firmware (preferred) |
| **[SHA256SUMS.txt](./SHA256SUMS.txt)** | SHA-256 of every downloadable file (verify before run) |

Firmware: [merged.bin](./esp32-2432s028-sha256-miner-merged.bin) @ `0x0` · [app.bin](./esp32-2432s028-sha256-miner.bin) (Wi‑Fi OTA) · [VERSION.txt](./VERSION.txt)

### Windows security / verified files
- PE metadata embeds publisher **GutFarms** + product/version (Explorer Details).
- Compare `Get-FileHash <file> -Algorithm SHA256` to [SHA256SUMS.txt](./SHA256SUMS.txt).
- If SmartScreen blocks: Properties → **Unblock**, then re-check the hash.
- Optional Authenticode: set `WINDOWS_CODE_SIGN_PFX` (+ password) when building for a Verified publisher signature.
