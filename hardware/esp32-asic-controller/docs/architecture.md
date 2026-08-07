# JCHC-1 Architecture

## Goals

- Standalone WiFi stratum client (reuse this repo’s miner + pool stack).
- Safe, monitored power for one hashboard module.
- Clean digital interface so ASIC/FPGA details stay on the daughterboard.
- Bring-up without the ASIC (controller-only assembly).

## Block diagram

```
                    ┌─────────────────────────────────────────────┐
   USB-C 5V ───────►│  Ideal diode / polyfuse                     │
   Barrel 9-12V ───►│  VIN mux (prefer barrel when present)       │
                    │         │                                   │
                    │         ├──► Buck 5V (if VIN>5) ──► SYS_5V  │
                    │         └──► SYS_5V (USB path)              │
                    │                │                            │
                    │    ┌───────────┼──────────────┐             │
                    │    ▼           ▼              ▼             │
                    │  LDO 3V3     Buck VCORE     Buck VIO        │
                    │  (ESP+IO)    0.6–1.2V/15A   1.8V/1A         │
                    │    │           │              │             │
                    │    ▼           ▼              ▼             │
                    │  ESP32-S3    J1 power pins   J1 VIO         │
                    │    │                                        │
                    │    ├── UART1 TX/RX ── TXS0108E ── J1 UART   │
                    │    ├── I2C: OLED, INA219, (EMC2101 opt)     │
                    │    ├── DAC/PWM trim ── VCORE setpoint       │
                    │    ├── GPIO: NRST, EN, FAN_PWM, LEDs        │
                    │    └── USB-Serial/JTAG                       │
                    └─────────────────────────────────────────────┘
                                      │
                                      ▼
                               J1 2×10 2.54mm
                               Hashboard module
```

## Power tree

| Rail | Source | Load | Notes |
|------|--------|------|-------|
| `VIN` | Barrel 9–12 V or USB 5 V | Whole board | Schottky/ideal-diode OR; polyfuse ~3 A on USB |
| `SYS_5V` | USB direct or buck from VIN | Fans, VCORE pre-reg, VIO pre-reg | Never feed 12 V into USB |
| `3V3` | AMS1117-3.3 or AP2112K | ESP32, OLED, logic | ≥600 mA |
| `VCORE` | Sync buck (e.g. TPS543x / MP2315 + FET stage, or module) | ASIC core | Feedback + digipot/DAC trim; soft-start; UVLO |
| `VIO` | Buck/LDO 1.8 V | ASIC IO | JP1 selects 1.8 V vs 3.3 V |

**VCORE trim:** ESP32 DAC / filtered PWM → inject into buck feedback (Bitaxe-style DS4432 optional upgrade). Rev A uses filtered PWM + series resistor into FB for simplicity.

**Current sense:** INA219 on `SYS_5V` high-side (or INA260 if preferred). Firmware polls over I2C.

## Digital

| Function | ESP32-S3 GPIO | Notes |
|----------|---------------|-------|
| HASH_TX | GPIO17 | UART1 TX → level shifter OE side A |
| HASH_RX | GPIO18 | UART1 RX ← level shifter |
| HASH_NRST | GPIO8 | Active-low reset to hashboard |
| HASH_EN | GPIO9 | Enable / power-good gate |
| I2C_SDA | GPIO5 | OLED + INA219 |
| I2C_SCL | GPIO6 | 4k7 pull-ups to 3V3 |
| FAN_PWM | GPIO7 | 25 kHz PWM |
| FAN_TACH | GPIO10 | Input pull-up |
| VCORE_PWM | GPIO11 | Filter → buck FB trim |
| LED_PWR | GPIO12 | Green |
| LED_HASH | GPIO13 | Blue activity |
| LED_ERR | GPIO14 | Red |
| BOOT | GPIO0 | Strap / button |
| OLED_RST | GPIO21 | Optional |

UART0 left free for debug pads. USB D+/D− to ESP32-S3 native USB.

Baud default: **1_500_000** 8N1 (configurable; 115200 for bring-up).

## Mechanical / thermal

- 100 × 70 mm outline; M3 mounting holes at corners (inset 3.5 mm).
- Hashboard mounts above or beside via J1 + optional standoffs.
- Keep VCORE inductor / FET copper pour on bottom; thermal vias under regulator.
- Fan blows across hashboard; controller stays cooler.

## Layers (recommended)

| Layer | Use |
|-------|-----|
| L1 | Signals + component |
| L2 | GND plane |
| L3 | 3V3 / VCORE pours |
| L4 | Signals + GND pour |

2-layer OK for **controller bring-up without ASIC load**; use 4-layer before >5 A VCORE.

## Related designs

Topology borrows ideas from open Bitcoin miners (Bitaxe: ESP32-S3 + buck + UART ASIC). JCHC-1 does **not** copy BM13xx footprints or proprietary command sets; the hasher protocol is this project’s own framing for scrypt-class jobs.
