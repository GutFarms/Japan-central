# Flash images & web flasher

## Drag & drop (recommended)

```bash
./scripts/build-flash-images.sh
./scripts/serve-web-flasher.sh
```

Open **http://127.0.0.1:8080/web/** → drop `esp32-2432s028-scrypt-miner-merged.bin` → **Connect & flash**.

Files: [`web/`](web/)

## CLI images

Run from repo root:

```bash
./scripts/build-flash-images.sh
```

Then see [`../FLASH.md`](../FLASH.md).
