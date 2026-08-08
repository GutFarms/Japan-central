# Metrics protocol v1

JSON metrics from the PC host / desktop app to the ESP32-CYD firmware.

## Transports

### USB serial

| Field | Value |
| --- | --- |
| Baud | `115200` |
| Framing | NDJSON (`\n` terminated) |
| Max line | ≤ 512 bytes |

On connect the host may send `{"v":1,"hello":1}`; firmware replies  
`{"ok":1,"fw":"cyd-monitor","proto":1}`.

Each accepted metrics line is ACKed as `{"ok":1,"seq":N}`.

### Wi‑Fi UDP

| Field | Value |
| --- | --- |
| Port | `4210` |
| ACK | firmware replies `{"ok":1,"seq":N}` to the sender |

## Schema

```json
{
  "v": 1,
  "seq": 12,
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

| Key | Notes |
| --- | --- |
| `seq` | Monotonic packet id (host → device), echoed in ACK |
| metric keys | Missing keys keep previous on-device values (delta updates OK) |
| full snapshot | Host sends a full object every ~8 packets |

## Link quality

Host measures RTT from ACK timestamps and shows ACK ratio in the desktop app.  
Firmware shows approximate packets/sec in the footer.
