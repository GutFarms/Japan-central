# Metrics protocol v1

JSON metrics from the PC host agent to the ESP32-CYD firmware.

## Transports

Both transports share the same JSON schema. Use either or both at once.

### USB serial (recommended for direct PC link)

| Field | Value |
| --- | --- |
| Interface | USB CDC / UART (`Serial` on the CYD) |
| Baud | `115200` |
| Framing | **NDJSON** — one UTF-8 JSON object per line, terminated by `\n` |
| Max line | ≤ 512 bytes |
| Suggested rate | 2 Hz (every 500 ms) |
| ACK | Firmware replies with `{"ok":1}` after a valid line |

Host examples:

```bash
python agent.py --serial COM3
python agent.py --serial /dev/ttyUSB0
python agent.py --serial auto
python agent.py --list-ports
```

### Wi‑Fi UDP

| Field | Value |
| --- | --- |
| Protocol | UDP |
| Default port | `4210` |
| Encoding | UTF-8 JSON, single object per datagram |
| Max size | ≤ 512 bytes |
| Suggested rate | 2 Hz (every 500 ms) |

```bash
python agent.py --host 192.168.1.50
```

USB + UDP together:

```bash
python agent.py --serial auto --host 192.168.1.50
```

## Schema

```json
{
  "v": 1,
  "cpu": 42.5,
  "cpu_temp": 61.0,
  "ram": 58.2,
  "gpu": 71.0,
  "gpu_temp": 68.0,
  "vram": 44.0,
  "fps": 144,
  "host": "DESKTOP"
}
```

| Key | Type | Unit | Notes |
| --- | --- | --- | --- |
| `v` | int | — | Protocol version (`1`) |
| `cpu` | number | % | 0–100 CPU utilization |
| `cpu_temp` | number | °C | `0` if unavailable |
| `ram` | number | % | 0–100 system RAM used |
| `gpu` | number | % | 0–100 GPU utilization |
| `gpu_temp` | number | °C | `0` if unavailable |
| `vram` | number | % | 0–100 VRAM used |
| `fps` | int | fps | Optional; `0` hides FPS in footer |
| `host` | string | — | Max 23 chars; label in header |

Unknown keys are ignored. Missing keys keep the previous on-device value.

## Stale link

If no valid packet arrives for **3 seconds**, the firmware shows `WAIT` and keeps the last painted values until the next packet. The footer shows `USB serial` or `WiFi UDP` based on the last successful source.
