# Bring-up checklist

## Before first power

1. Visual inspect: shorts on USB / VCORE inductor / J1.
2. No hashboard attached.
3. JP1 = 1.8 V (default) or 3.3 V for FPGA stub.
4. Current-limited supply: USB via meter, or bench 5 V on SYS_5V test point.

## Power-only

1. Apply 5 V USB. LED_PWR on. Measure `3V3` = 3.25–3.35 V.
2. Measure `VIO` ≈ 1.8 V (or 3.3 V).
3. With VCORE enable **off**, `VCORE` ≈ 0 V.
4. Enable VCORE in firmware at lowest trim; expect ~0.6–0.8 V empty (no ASIC). Do not exceed 1.2 V unloaded for long.

## ESP32

1. Hold BOOT, reset, flash merged binary (USB-JTAG/serial).
2. Serial 115200: credentials flow as in main README.
3. OLED shows boot splash / mine tab.
4. `hello` on hasher UART (logic analyzer on J1-11/12) when `asic` feature enabled.

## With FPGA stub hashboard

1. Power off. Mate J1. Power on.
2. Expect INFO frame after HELLO; caps bit8 set.
3. Submit demo job; expect SHARE frames; stratum submit path unchanged.

## With real ASIC hashboard (future)

1. Read ASIC datasheet for absolute max VCORE / sequencing.
2. Re-validate buck current limit and compensation.
3. Start at minimum frequency/voltage; watch INA219 and thermals.
4. Only then raise clock.

## Fail-safe

- Firmware must drop `EN` and VCORE PWM to minimum if INA219 current &gt; limit or temp &gt; threshold.
- Hardware UVLO on buck; polyfuse on USB.
