"""Render tools/icon_src/*.svg to PNG (1x and @2x) inside the plugin's imgs folder."""
import sys
from pathlib import Path

import fitz

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "tools" / "icon_src"
OUT = ROOT / "plugin" / "com.xiaodoudou.helldivers2-stratagems.sdPlugin" / "imgs"
SIZES = {"scan": 72, "clear": 72, "acquiring": 72, "scan_icon": 20, "clear_icon": 20, "acquiring_icon": 20}


def main() -> int:
    for name, size in SIZES.items():
        page = fitz.open(str(SRC / f"{name}.svg"))[0]
        for suffix, mult in (("", 1), ("@2x", 2)):
            px = size * mult
            z = px / page.rect.width
            page.get_pixmap(matrix=fitz.Matrix(z, z), alpha=True).save(str(OUT / f"{name}{suffix}.png"))
    print("ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
