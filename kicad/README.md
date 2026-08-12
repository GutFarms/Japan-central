# KiCad project — JCHC-1

Open **`jchc1.kicad_pro`** in **KiCad 8+**.

## What’s generated

| File | Contents |
|------|----------|
| `jchc1.kicad_sch` | Logical schematic (block + net legend) |
| `jchc1.kicad_pcb` | 100×70 mm outline, M3 holes, major parts placed, GND/VCORE/3V3 zones |
| `board-preview.svg` | Quick visual of placement |

## Before ordering PCBs

1. Replace placeholder module footprints with exact library parts (ESP32-S3-WROOM-1, USB-C, regulators).
2. **Schematic Editor → Annotate → Assign footprints** from `../manufacturing/bom.csv`.
3. **Update PCB from Schematic**, then route remaining signals (UART, I2C, USB).
4. Run DRC. Export Gerbers to `../manufacturing/gerbers/`.

The PCB file is a **placement + plane starting point**, not a finished DRC-clean route. Expect a short KiCad session to finish nets before fab.
