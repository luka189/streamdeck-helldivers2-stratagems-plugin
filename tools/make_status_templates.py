"""Cut greyscale status templates (state words, arrow glyphs and timer digits) from reference frames at 2560x1440.

    python tools/make_status_templates.py menu_open.png menu_unavailable.png menu_activating.png capture_2560x1440_082327.png

The frames are the screenshots of the same names in data/samples/ (not in git).
"""
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "data" / "status"

ROW0, PITCH = 108, 70
BAND = (34, 59)  # status line, relative to the row's icon top
WORD_X = (154, 212)  # first ~58 px of the status line: enough to tell the words apart
ARROW_X, ARROW_STEP, ARROW_W = 155, 29, 26
# "Cooldown T-MM:SS": the timer's digit cells (left edge of each cut), 12 px each; the colon sits between
# the minutes and the seconds. "Inbound T-MM:SS" has the same cells 16 px further left.
COOLDOWN_DIGITS = (287, 300, 318, 331)
DIGIT_W = 12


def band(im, k, x0, x1):
    top = ROW0 + PITCH * k
    return im.crop((x0, top + BAND[0], x1, top + BAND[1])).convert("L")


def main(open_frame: str, unavailable_frame: str, activating_frame: str, timers_frame: str) -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    a, b, c, d = Image.open(open_frame), Image.open(unavailable_frame), Image.open(activating_frame), Image.open(timers_frame)
    arrow = lambda im, k, i: band(im, k, ARROW_X + ARROW_STEP * i, ARROW_X + ARROW_STEP * i + ARROW_W)
    digit = lambda im, k, i, shift=0: band(im, k, COOLDOWN_DIGITS[i] + shift, COOLDOWN_DIGITS[i] + shift + DIGIT_W)
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
        "digit_0": digit(a, 4, 0),  # Quasar Cannon: Cooldown T-04:50
        "digit_4": digit(a, 4, 1),
        "digit_5": digit(a, 4, 2),
        "digit_8": digit(a, 5, 3),  # Warp Pack: Cooldown T-04:48
        "digit_1": digit(b, 2, 2),  # Resupply: Cooldown T-00:16
        "digit_6": digit(b, 2, 3),
        "digit_7": digit(b, 5, 3, -16),  # Warp Pack: Inbound T-00:07
        "digit_3": digit(d, 4, 1),  # Quasar Cannon: Cooldown T-03:08
        "digit_2": digit(d, 5, 1),  # Warp Pack: Cooldown T-02:59
        "digit_9": digit(d, 5, 3),
    }
    for name, img in cuts.items():
        img.save(OUT / f"{name}.png")
    print(", ".join(f"{n} {i.size[0]}x{i.size[1]}" for n, i in cuts.items()))
    return 0


if __name__ == "__main__":
    sys.exit(main(*sys.argv[1:5]))
