"""Cut greyscale status templates (state words and arrow glyphs) from two reference frames at 2560x1440."""
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "data" / "status"

ROW0, PITCH = 108, 70
BAND = (34, 59)  # status line, relative to the row's icon top
WORD_X = (154, 212)  # first ~58 px of the status line: enough to tell the words apart
ARROW_X, ARROW_STEP, ARROW_W = 155, 29, 26


def band(im, k, x0, x1):
    top = ROW0 + PITCH * k
    return im.crop((x0, top + BAND[0], x1, top + BAND[1])).convert("L")


def main(open_frame: str, unavailable_frame: str, activating_frame: str) -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    a, b, c = Image.open(open_frame), Image.open(unavailable_frame), Image.open(activating_frame)
    arrow = lambda im, k, i: band(im, k, ARROW_X + ARROW_STEP * i, ARROW_X + ARROW_STEP * i + ARROW_W)
    cuts = {
        "word_ready": band(a, 0, *WORD_X),
        "word_impact": band(a, 3, *WORD_X),
        "word_cooldown": band(a, 4, *WORD_X),
        "word_unavailable": band(b, 3, *WORD_X),
        "word_inbound": band(b, 5, *WORD_X),
        "word_activating": band(c, 3, *WORD_X),
        "arrow_up": arrow(a, 1, 0),  # SoS Beacon: up down right up
        "arrow_down": arrow(a, 1, 1),
        "arrow_right": arrow(a, 1, 2),
        "arrow_left": arrow(a, 6, 0),  # Gunner FRV starts with left
    }
    for name, img in cuts.items():
        img.save(OUT / f"{name}.png")
    print(", ".join(f"{n} {i.size[0]}x{i.size[1]}" for n, i in cuts.items()))
    return 0


if __name__ == "__main__":
    sys.exit(main(*sys.argv[1:4]))
