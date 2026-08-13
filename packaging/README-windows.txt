Njörðr Seas' CYD miner — USB SHA-256 Bitcoin miner (app-only notes)
==========================================================

Prefer the full kit installer when possible:
  CYD-Miner-Setup.exe  (app + firmware + flash helper)

Quick use
---------
1. Flash esp32-2432s028-sha256-miner-merged.bin @ 0x0 (see FLASH-WINDOWS.txt)
2. Plug USB-C
3. Run cyd-companion.exe
4. Select COM port → Connect
5. Stratum stratum+tcp://btc.hmpool.io:3337
6. Worker = Bitcoin address, password = x
7. Start mining

Board has no Wi-Fi. Expect tens–hundreds of kH/s. Solo BTC is a lottery.

SmartScreen / unverified file
-----------------------------
If Windows warns about an unknown publisher: Properties → Unblock, then
verify SHA-256 against flash/downloads/SHA256SUMS.txt (or Firmware\SHA256SUMS.txt).
True "Verified publisher" needs an Authenticode certificate on the release build.
