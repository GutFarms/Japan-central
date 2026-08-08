# Metrics protocol v1

UDP JSON packets from the PC host agent to the ESP32-CYD firmware.

## Transport

| Field | Value |
| --- | --- |
| Protocol | UDP |
| Default port | `4210` |
| Encoding | UTF-8 JSON, single object per datagram |
| Max size | ≤ 512 bytes |
| Suggested rate | 2 Hz (every 500 ms) |

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

If no valid packet arrives for **3 seconds**, the firmware shows `WAIT` and keeps the last painted values until the next packet.
