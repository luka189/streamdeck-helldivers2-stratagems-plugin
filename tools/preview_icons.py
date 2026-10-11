"""Render the plugin SVG icons side by side for a quick visual check."""
import sys
from pathlib import Path

import fitz
from PIL import Image

IMGS = Path(__file__).resolve().parent / "icon_src"
names = ["scan", "clear", "acquiring"]
sheet = Image.new("RGB", (144 * 3 + 40, 164), (60, 60, 60))
for i, n in enumerate(names):
    page = fitz.open(str(IMGS / f"{n}.svg"))[0]
    z = 144 / page.rect.width
    pix = page.get_pixmap(matrix=fitz.Matrix(z, z), alpha=False)
    sheet.paste(Image.frombytes("RGB", (pix.width, pix.height), pix.samples), (10 + i * 154, 10))
out = sys.argv[1]
sheet.save(out)
print(out)
