"""Render tools/icon_src/*.svg to PNG (1x and @2x) inside the plugin's imgs folder."""
import io
import sys
from pathlib import Path

import fitz
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "tools" / "icon_src"
OUT = ROOT / "plugin" / "com.xiaodoudou.helldivers2-stratagems.sdPlugin" / "imgs"
SIZES = {"plugin": 256, "category": 28, "scan": 72, "clear": 72, "acquiring": 72, "setup": 72, "scan_icon": 20, "clear_icon": 20, "acquiring_icon": 20, "setup_icon": 20}
# Icons of the Stream Deck actions list: Elgato's guidelines want them white (#FFFFFF) on transparent. Antialiased
# edges come out of the renderer as 254 grey here and there, so their colour is set to pure white, keeping the alpha.
WHITE_ONLY = {"category", "scan_icon", "clear_icon", "acquiring_icon", "setup_icon"}


def main() -> int:
    for name, size in SIZES.items():
        page = fitz.open(str(SRC / f"{name}.svg"))[0]
        for suffix, mult in (("", 1), ("@2x", 2)):
            px = size * mult
            z = px / page.rect.width
            png = page.get_pixmap(matrix=fitz.Matrix(z, z), alpha=True).tobytes("png")
            img = Image.open(io.BytesIO(png)).convert("RGBA")
            if name in WHITE_ONLY:
                img = Image.merge("RGBA", (*Image.new("RGB", img.size, (255, 255, 255)).split(), img.getchannel("A")))
            img.save(OUT / f"{name}{suffix}.png")
    print("ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
