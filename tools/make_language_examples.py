"""Build the annotated English examples of docs/TRANSLATIONS.md from the full 1440p screenshots in data/samples
(not in git: they are several MB each)."""
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
SAMPLES = ROOT / "data" / "samples"
OUT = ROOT / "docs" / "img" / "translations"
INK = (255, 210, 63)
ORIGIN = (30, 70)  # crop corner in the 2560x1440 frame
# status line of menu row k, in crop coordinates
ROW = lambda k: (118, 68 + 70 * k, 340, 101 + 70 * k)  # noqa: E731

# sample, crop height, [(row, label)], output name
EXAMPLES = [
    ("menu_open.png", 540, [(0, "Ready"), (3, "Impact"), (4, "Cooldown"), (5, "Cooldown")], "menu_ready_impact_cooldown.png"),
    ("menu_unavailable.png", 540, [(0, "Ready"), (2, "Cooldown"), (3, "Unavailable"), (5, "Inbound")], "menu_unavailable_inbound.png"),
    ("menu_activating.png", 330, [(3, "Activating")], "closed_activating.png"),
]


def font(size: int) -> ImageFont.FreeTypeFont:
    for name in ("segoeuib.ttf", "arialbd.ttf"):
        try:
            return ImageFont.truetype(name, size)
        except OSError:
            pass
    return ImageFont.load_default()


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    label_font = font(22)
    for sample, height, marks, name in EXAMPLES:
        im = Image.open(SAMPLES / sample).convert("RGB")
        crop = im.crop((ORIGIN[0], ORIGIN[1], ORIGIN[0] + 560, ORIGIN[1] + height))
        d = ImageDraw.Draw(crop)
        for row, label in marks:
            x0, y0, x1, y1 = ROW(row)
            d.rectangle((x0, y0, x1, y1), outline=INK, width=3)
            tx, ty = x1 + 12, y0 + 2
            box = d.textbbox((tx, ty), label, font=label_font)
            d.rectangle((box[0] - 6, box[1] - 4, box[2] + 6, box[3] + 4), fill=(20, 18, 10))
            d.text((tx, ty), label, fill=INK, font=label_font)
        crop.save(OUT / name, optimize=True)
        print(name, crop.size)
    return 0


if __name__ == "__main__":
    sys.exit(main())
