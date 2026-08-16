# Njörðr Seas' CYD miner

USB / Wi‑Fi control for the ESP32-2432S028 SHA-256 miner.

- Boards mine **independently** to the pool when STA + pool are set
- Companion monitors H/s, pushes OTA, and feeds jobs only while `!mine_indep`
- SoftAP setup at `http://10.88.88.1/`

```bash
./scripts/build-companion-windows.sh
```
