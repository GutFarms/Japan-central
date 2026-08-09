# ESP32-2432S028 (CYD) — C++ companion firmware

Companion-first firmware for the Cheap Yellow Display. UART0 speaks only the
`cmp` protocol used by **CYD Companion** — no classic serial field prompts.

## Protocol

| Command | Reply |
|---------|--------|
| `cmp ping` | `CMP ok usb` |
| `cmp status` | `CMPSTATUS {…}` |
| `cmp config` | `CMPCONFIG {…}` |
| `cmp set …` | `CMPACK queued` / `CMPERR …` |
| `cmp clock cpu_mhz=240` | same |
| `cmp reboot` | same |

## Build

```bash
pio run -d firmware-cpp
```

Outputs (via post script):

- `flash/esp32-2432s028-scrypt-miner.bin`
- `flash/esp32-2432s028-scrypt-miner-merged.bin` — flash at **0x0**, DIO, 4 MB, 40 MHz

## Board

- ESP32-2432S028 (ILI9341, CH340 USB-UART)
- CPU default 240 MHz, hash-focus on
- Litecoin-style scrypt N=1024 with TMTO (64 checkpoints)
