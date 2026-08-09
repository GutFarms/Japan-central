CYD Companion — quick start
===========================

1. Flash the C++ firmware (merged.bin @ 0x0) if you have not already.
2. Plug the CYD USB cable (CH340 COM port).
3. Run cyd-companion.exe
4. Select COM port → Connect
5. Enter WiFi SSID/password, stratum, worker, pool password
6. Save & reboot

The board only shows mining stats + a network ticker. All control is in this app.
Markets prices are fetched on the PC and pushed to the board over USB.

USB protocol: cmp ping | status | config | set | clock | reboot | netdata
