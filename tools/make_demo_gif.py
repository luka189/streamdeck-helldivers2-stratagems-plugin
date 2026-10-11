"""Turn the key images of a scripted mission into docs/img/demo.gif, drawn as a Stream Deck.

    cd scanner
    cargo test --bin hd2-plugin -- --ignored --nocapture demo_frames   # writes scanner/target/demo/frames.json
    cd ..
    python tools/make_demo_gif.py

The key images come from the plugin's own painting code, so the GIF shows exactly what the keys show.
Needs Pillow and PyMuPDF (to draw the SVG keys).
"""
import base64
import io
import json
import sys
from pathlib import Path

import fitz
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
FRAMES = ROOT / "scanner" / "target" / "demo" / "frames.json"
OUT = ROOT / "docs" / "img" / "demo.gif"
KEY, GAP, MARGIN, RADIUS = 96, 14, 22, 14
DECK, BODY = (14, 14, 16), (32, 32, 36)
PRESSED = 0.9  # a pressed key is drawn this much smaller, for one frame


def key_image(url: str, cache: dict) -> Image.Image:
    if url not in cache:
        head, data = url.split(",", 1)
        raw = base64.b64decode(data)
        if head.startswith("data:image/svg+xml"):
            page = fitz.open("svg", raw)[0]
            z = KEY / page.rect.width
            img = Image.open(io.BytesIO(page.get_pixmap(matrix=fitz.Matrix(z, z), alpha=False).tobytes("png")))
        else:
            img = Image.open(io.BytesIO(raw))
        cache[url] = img.convert("RGB").resize((KEY, KEY), Image.LANCZOS)
    return cache[url]


def rounded(img: Image.Image) -> tuple[Image.Image, Image.Image]:
    mask = Image.new("L", img.size, 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, img.width - 1, img.height - 1), radius=RADIUS * img.width // KEY, fill=255)
    return img, mask


def main() -> int:
    spec = json.loads(FRAMES.read_text(encoding="utf-8"))
    cols, frame_ms = spec["columns"], spec["frame_ms"]
    rows = -(-len(spec["frames"][0]["keys"]) // cols)
    w = 2 * MARGIN + cols * KEY + (cols - 1) * GAP
    h = 2 * MARGIN + rows * KEY + (rows - 1) * GAP
    cache: dict = {}
    images, durations = [], []
    for frame in spec["frames"]:
        canvas = Image.new("RGB", (w, h), DECK)
        ImageDraw.Draw(canvas).rounded_rectangle((4, 4, w - 5, h - 5), radius=24, fill=BODY)
        for i, url in enumerate(frame["keys"]):
            x = MARGIN + (i % cols) * (KEY + GAP)
            y = MARGIN + (i // cols) * (KEY + GAP)
            img = key_image(url, cache)
            if frame["pressed"] == i:
                size = int(KEY * PRESSED)
                img = img.resize((size, size), Image.LANCZOS)
                x, y = x + (KEY - size) // 2, y + (KEY - size) // 2
            img, mask = rounded(img)
            canvas.paste(img, (x, y), mask)
        # identical frames in a row become one longer frame
        if images and list(images[-1].getdata()) == list(canvas.getdata()):
            durations[-1] += frame_ms
        else:
            images.append(canvas)
            durations.append(frame_ms)
    durations[-1] += 1500  # rest on the last frame before looping
    OUT.parent.mkdir(parents=True, exist_ok=True)
    images[0].save(OUT, save_all=True, append_images=images[1:], duration=durations, loop=0, optimize=True, disposal=1)
    print(f"{OUT}: {len(images)} frames, {OUT.stat().st_size / 1024:.0f} KB")
    return 0


if __name__ == "__main__":
    sys.exit(main())
