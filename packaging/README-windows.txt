CYD Companion — USB scrypt miner control
========================================

1. Flash esp32-2432s028-scrypt-miner-merged.bin @ 0x0
2. Plug the board with USB-C
3. Run cyd-companion.exe
4. Select COM port → Connect
5. Enter stratum URL, worker (Litecoin address), password (d=1)
6. Start mining

The board has no Wi-Fi. Pool traffic stays on this PC.
Work is sent over USB; shares come back over USB.
