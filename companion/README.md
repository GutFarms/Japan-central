# CYD Companion (Windows, C++)

Native Win32 control app for the ESP32-2432S028 scrypt miner. The board only mines
and shows a basic stats screen — **all setup and control happens here**.

## Build

```bash
make -C companion
# → dist/cyd-companion-windows/cyd-companion.exe
./scripts/build-companion-windows.sh
```

## Use

1. Flash C++ firmware (`./scripts/build-flash-images.sh`)
2. Plug USB (CH340)
3. Run `cyd-companion.exe` → pick COM → **Connect**
4. Fill WiFi / Pool → **Save & reboot**
