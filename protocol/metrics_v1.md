# Metrics protocol v1

JSON metrics from the PC host agent / desktop app to the ESP32-CYD firmware.

## Transports

### USB serial (recommended)

| Field | Value |
| --- | --- |
| Interface | USB CDC / UART |
| Baud | `115200` (configurable in app Settings) |
| Framing | NDJSON — one UTF-8 JSON object per line (`\n`) |
| Max line | ≤ 512 bytes |

### Wi‑Fi UDP

| Field | Value |
| --- | --- |
| Protocol | UDP |
| Default port | `4210` |
| Encoding | UTF-8 JSON datagram |

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
  "disk": 62.0,
  "swap": 10.0,
  "net_up": 1.25,
  "net_down": 8.5,
  "fps": 0,
  "host": "DESKTOP"
}
```

| Key | Type | Unit | Notes |
| --- | --- | --- | --- |
| `v` | int | — | Protocol version (`1`) |
| `cpu` / `gpu` / `ram` / `vram` / `disk` / `swap` | number | % | 0–100 |
| `cpu_temp` / `gpu_temp` | number | °C | `0` if unavailable |
| `net_up` / `net_down` | number | Mbps | instantaneous rates |
| `fps` | int | fps | optional |
| `host` | string | — | max 23 chars |

Unknown keys are ignored. Missing keys keep previous on-device values.

The desktop app may collect richer local fields (`cpu_mhz`, `ram_used_gb`, …) for its own dials; the wire payload stays compact for the CYD.
