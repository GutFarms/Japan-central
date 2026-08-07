# Fabrication notes — JCHC-1 Rev A

## Deliverables to order

From KiCad (`kicad/jchc1.kicad_pro`):

1. Gerbers (RS-274X) + Excellon drill
2. Board outline / fab drawing
3. BOM CSV (`bom.csv`)
4. CPL / pick-and-place (`cpl.csv`) — refine after final placement in KiCad GUI
5. Stackup note (below)

## Suggested fab options

| Vendor path | Stackup | Finish | Qty |
|-------------|---------|--------|-----|
| JLCPCB 4-layer | 1.6 mm, 1 oz outer / 0.5 oz inner | HASL lead-free or ENIG | 5 |
| JLCPCB 2-layer | 1.6 mm, 1 oz | HASL | 5 (controller bring-up only) |

Min track/clearance: **0.15 mm / 0.15 mm**.  
VCORE pours: **≥2 mm** effective copper width per amp; use planes + many vias.

## Assembly

- PCBA: JLCPCB assembly for basic parts; **hand-solder** ESP32-S3 module, inductors, and J1 if not in LCSC lib.
- Stencil: 0.12 mm recommended for module.
- Do not assemble hashboard ASIC until datasheeted.

## Test points (silkscreen)

| TP | Net |
|----|-----|
| TP1 | VIN |
| TP2 | SYS_5V |
| TP3 | 3V3 |
| TP4 | VCORE |
| TP5 | VIO |
| TP6 | GND |

## After fab

1. Follow `docs/bringup.md`.
2. Flash firmware with `asic` feature.
3. Capture UART hello with logic analyzer before mating expensive silicon.
