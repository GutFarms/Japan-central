# JCHC-1 — ESP32 ASIC Hash Controller (hardware only)

**Standalone board package** for later manufacturing. This is **not** the miner firmware — firmware lives on separate branches/PRs (e.g. CYD / ESP32-S3 scrypt miner).

```
  USB-C / 12V ──► Power tree ──► ESP32-S3 ──UART──► Hashboard connector ──► ASIC/FPGA
                       │              │
                       │              ├── WiFi stratum (via your firmware)
                       │              ├── OLED status
                       │              └── Fan / INA / VCORE DAC
                       └── VCORE + VIO to hashboard
```

## Why modular?

Open hobbyist ASICs such as Bitaxe **BM13xx** are **SHA-256 (Bitcoin)**, not scrypt. There is **no** widely documented drop-in open **scrypt** ASIC comparable to BM1366 today. JCHC-1 therefore:

1. Implements a **Bitaxe-class controller** (ESP32 + power + monitoring + UART).
2. Exposes a **fixed 20-pin hashboard connector** so you can fab a daughterboard later when you source scrypt silicon, commission a custom die, or bring up an **FPGA hasher** for protocol testing.
3. Documents a **Hasher UART protocol** that matches an 80-byte header + LE target + nonce share shape used by typical stratum scrypt miners.

Do **not** populate an ASIC footprint on this revision until you have a datasheeted part.

## What’s in this folder

| Path | Purpose |
|------|---------|
| `docs/architecture.md` | Block diagram, rails, clocking, thermal |
| `docs/connector.md` | J1 hashboard pinout + electrical limits |
| `docs/hasher-uart-protocol.md` | ESP32 ↔ hasher framing (job in / nonce out) |
| `docs/bringup.md` | First-power checklist |
| `kicad/` | KiCad 8 schematic + PCB (open in KiCad) |
| `manufacturing/` | BOM, CPL hints, fab notes, JLCPCB checklist |
| `firmware-stub/` | Optional standalone HUP codec crate (not the miner app) |

## Board summary

| Item | Spec |
|------|------|
| Name / rev | JCHC-1 / A |
| MCU | ESP32-S3-WROOM-1 (16N8R recommended) |
| Size | 100 × 70 mm, 4-layer preferred (2-layer possible for bring-up) |
| Input | USB-C 5 V (data + 5 V power) **and** optional 5.5×2.1 mm barrel 9–12 V |
| Logic | 3.3 V |
| ASIC core | Adjustable buck **VCORE** 0.60–1.20 V, design target **15 A** continuous |
| ASIC IO | **VIO** 1.8 V @ 1 A (selectable 3.3 V via jumper) |
| Hasher link | UART 3.3 V ↔ TXS0108E level shift ↔ hashboard |
| Sense | INA219 (12 V/5 V input current), NTC + I2C temp header |
| Cooling | 4-pin fan (PWM + TACH), MOSFET soft-start |
| UI | 0.91″ SSD1306 I2C OLED, 3× status LED, BOOT/RESET |
| Prog | USB-C native USB (ESP32-S3), UART0 test pads |

## Relation to firmware

- Flash **your** stratum/scrypt firmware (separate repo path / PR) onto the ESP32-S3.
- Point that firmware’s hasher UART at **GPIO17 TX / GPIO18 RX** (see `docs/architecture.md`).
- Optional: use `firmware-stub/` as a reference codec when wiring the miner to J1 — it is **not** a complete mining application.

## Manufacturing later

See `manufacturing/FAB.md`. Typical flow: review KiCad → export Gerbers + drill + BOM/CPL → JLCPCB/PCBWay → assemble controller → fab hashboard when ASIC/FPGA is chosen → flash firmware from the miner project.

## License

Hardware docs and KiCad files: `MIT OR Apache-2.0`. Provided as-is for personal/experimental fab; verify every rail before connecting silicon.
