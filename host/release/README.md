# CYD Monitor — downloadable host app

Auto-connects to an ESP32-CYD over USB and streams PC/GPU metrics.

## Options

### 1) Portable zip (Windows / Linux / macOS)

Download **`CYD-Monitor-portable.zip`**, unzip, then:

- **Windows:** double-click `CYD Monitor.bat`  
  (first run installs a local `.venv` — needs Python 3 from python.org)
- **Linux/macOS:**
  ```bash
  chmod +x CYD-Monitor.sh
  ./CYD-Monitor.sh
  ```

### 2) Standalone binary (Linux)

Run **`CYD-Monitor`** directly (no Python install needed):

```bash
chmod +x CYD-Monitor
./CYD-Monitor
```

> Windows `.exe` builds need to be produced on a Windows machine:
> `build_app.sh` / PyInstaller with the same `desktop_app.py` entrypoint.

## Behavior

1. Scans USB serial ports for CYD-like adapters (CP210x, CH340, CH9102, FTDI, Espressif)
2. Opens the best match at **115200** baud
3. Streams NDJSON metrics every 0.5s
4. Auto-reconnects if the cable is unplugged

Plug the CYD in over USB, launch the app, and leave it running.
