CYD Companion — quick start
===========================

1. Flash the C++ firmware (merged.bin @ 0x0) if you have not already.
2. Plug the CYD USB cable (CH340 COM port).
3. Run cyd-companion.exe
4. Select COM port → Connect
5. Enter WiFi SSID/password
6. Pool defaults: scrypt.mysolopool.com:3341 · worker = LTC address · password d=1
7. Save & reboot

LTC+DOGE merged mining (same scrypt H/s; low d=1 so ESP32 shares can be accepted).
The board shows mining stats + a network ticker. Markets prices are pushed over USB.

USB protocol: cmp ping | status | config | set | clock | reboot | netdata
