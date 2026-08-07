# J1 — Hashboard connector

**Part:** 2×10 male pin header, 2.54 mm pitch, on controller.  
**Mate:** Female 2×10 on hashboard (or IDC ribbon ≤10 cm for lab use).

Pin 1 is marked with a square pad and silkscreen triangle.

```
        Controller top view (J1)
   ┌────────────────────────────┐
   │  2  4  6  8 10 12 14 16 18 20 │
   │  1  3  5  7  9 11 13 15 17 19 │
   └────────────────────────────┘
        pin1 ■
```

| Pin | Name | Dir | Level | Max | Description |
|-----|------|-----|-------|-----|-------------|
| 1 | VCORE | PWR | 0.6–1.2 V | 15 A | ASIC core (many pins paralleled on PCB pour → pins 1,3,5) |
| 3 | VCORE | PWR | | | |
| 5 | VCORE | PWR | | | |
| 2 | GND | PWR | 0 V | 15 A | Return (pins 2,4,6,8) |
| 4 | GND | PWR | | | |
| 6 | GND | PWR | | | |
| 8 | GND | PWR | | | |
| 7 | VIO | PWR | 1.8 V (def) / 3.3 V | 1 A | ASIC IO / level-shifter B-side ref |
| 9 | 3V3_REF | PWR | 3.3 V | 100 mA | Optional logic on hashboard |
| 10 | PG_VCORE | IN | 3.3 V | | Power-good from hashboard (pull-up); leave NC if unused |
| 11 | UART_TX | OUT | 3.3 V→shifted | | Controller → hasher (CI) |
| 12 | UART_RX | IN | shifted→3.3 V | | Hasher → controller (RO) |
| 13 | NRST | OUT | open-drain 3.3 V | | Active-low reset |
| 14 | EN | OUT | 3.3 V | | Soft enable |
| 15 | SDA | I/O | 3.3 V | | Optional hashboard EEPROM/temp |
| 16 | SCL | OUT | 3.3 V | | |
| 17 | NTC | IN | analog | | 10k NTC to GND on hashboard (optional) |
| 18 | FAN_PWM | OUT | 3.3 V | | Pass-through / local fan |
| 19 | CLK_OPT | OUT | 3.3 V | | Optional reference clock |
| 20 | GND | PWR | | | Extra GND |

## Electrical rules

1. **Never** hot-plug J1 with VCORE enabled.
2. Sequence: `3V3` up → `VIO` up → `VCORE` ramp → release `NRST` → `EN` high → UART hello.
3. Level shifter **OE** tied to `VIO` present (or ESP GPIO) so A-side is Hi-Z until B-side powered.
4. If hashboard uses 1.8 V IO, set JP1 = 1.8 V and populate TXS0108E. If FPGA 3.3 V IO, set JP1 = 3.3 V and you may bypass shifter with 0 Ω links (pads provided).

## Hashboard stub (for later fab)

Minimum scrypt/FPGA module:

- Local decoupling on VCORE/VIO.
- Hasher UART engine implementing `docs/hasher-uart-protocol.md`.
- Optional AT24CS EEPROM with board ID string at I2C `0x50`.

FPGA bring-up suggestion: Lattice iCE40 / Xilinx Artix with UART job engine hashing scrypt-lite for protocol validation before real ASIC silicon.
