#!/usr/bin/env bash
# Regenerate installer icons / wizard bitmaps from logo_main.png
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/src/main/resources/logo_main.png"
OUT="$ROOT/installer/assets"
TMP="$OUT/tmp"
mkdir -p "$TMP"

for s in 16 24 32 48 64 128 256; do
  convert "$SRC" -resize "${s}x${s}" -background none -gravity center -extent "${s}x${s}" "$TMP/icon-${s}.png"
done
convert "$TMP"/icon-16.png "$TMP"/icon-24.png "$TMP"/icon-32.png \
  "$TMP"/icon-48.png "$TMP"/icon-64.png "$TMP"/icon-128.png "$TMP"/icon-256.png \
  "$OUT/icon.ico"
convert "$SRC" -resize 512x512 -background none -gravity center -extent 512x512 "$OUT/icon.png"
cp "$OUT/icon.ico" "$ROOT/icon.ico"
cp "$OUT/icon.png" "$ROOT/icon.png"

python3 - "$ROOT" <<'PY2'
import sys
from pathlib import Path
from PIL import Image, ImageDraw
root = Path(sys.argv[1])
logo = Image.open(root / "src/main/resources/logo_main.png").convert("RGBA")
out = root / "installer/assets"
LEAF, CHARCOAL, CANOPY = (42, 74, 54), (18, 26, 22), (243, 247, 241)

def gradient(size):
    im = Image.new("RGB", size, LEAF)
    w, h = size
    for y in range(h):
        t = y / max(h - 1, 1)
        c = tuple(int(LEAF[i] * (1 - t) + CHARCOAL[i] * t) for i in range(3))
        for x in range(w):
            im.putpixel((x, y), c)
    return im

def paste_logo(canvas, max_size, y):
    mark = logo.copy()
    mark.thumbnail(max_size, Image.Resampling.LANCZOS)
    canvas.paste(mark, ((canvas.width - mark.width) // 2, y), mark)

side = gradient((164, 314))
paste_logo(side, (120, 120), 40)
ImageDraw.Draw(side).rectangle([18, 200, 146, 280], fill=CANOPY)
side.save(out / "wizard-sidebar.bmp")

modern = gradient((192, 386))
paste_logo(modern, (140, 140), 70)
modern.save(out / "wizard-modern.bmp")

small = Image.new("RGB", (55, 55), CANOPY)
mark = logo.copy()
mark.thumbnail((48, 48), Image.Resampling.LANCZOS)
small.paste(mark, ((55 - mark.width) // 2, (55 - mark.height) // 2), mark)
small.save(out / "wizard-small.bmp")
print("assets refreshed")
PY2

rm -rf "$TMP"
echo "OK → $OUT"
