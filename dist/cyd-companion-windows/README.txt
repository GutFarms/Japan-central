CYD Companion for Windows
=========================

GPU (OpenGL) desktop app to control the ESP32-2432S028 scrypt miner over LAN.

Requirements
- Windows 10/11 x64
- Board flashed with firmware that exposes /api/config, /api/clock, /api/status
- Board on the same Wi‑Fi / LAN (note the IP from the LCD or serial)

Usage
1. Run cyd-companion.exe
2. Enter board IP (example 192.168.1.50)
3. Auth = pool password (same as stratum worker password)
4. Connect
5. Tabs: Dashboard · Settings · Overclock · Discover

Overclock
- 80 / 160 / 240 MHz CPU profiles
- Applied via soft-reset after save to flash

Notes
- WiFi setting changes reboot the board
- Keep auth password private on shared LANs (no TLS)
