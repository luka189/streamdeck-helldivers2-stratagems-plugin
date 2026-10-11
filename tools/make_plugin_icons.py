"""Generate the plugin's PNG assets from the wiki Reinforce icon."""
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "data" / "icons_144" / "Reinforce_Stratagem_Icon_Background.png"
OUT = ROOT / "plugin" / "com.xiaodoudou.helldivers2-stratagems.sdPlugin" / "imgs"

# name -> (size, background); each also written at 2x
ASSETS = {"plugin": (256, None), "category": (28, None)}


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    icon = Image.open(SRC).convert("RGBA")
    for name, (size, bg) in ASSETS.items():
        for suffix, mult in (("", 1), ("@2x", 2)):
            px = size * mult
            canvas = Image.new("RGBA", (px, px), bg or (0, 0, 0, 0))
            inner = round(px * (0.8 if bg else 1.0))
            glyph = icon.resize((inner, inner), Image.LANCZOS)
            canvas.alpha_composite(glyph, ((px - inner) // 2, (px - inner) // 2))
            canvas.save(OUT / f"{name}{suffix}.png")
    print("ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
