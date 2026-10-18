"""Make the Elgato Marketplace media into dist/store/: icon, thumbnail, gallery images and a video.

    cd scanner
    cargo test --bin hd2-plugin -- --ignored --nocapture store_scenes demo_frames
    cd ..
    python tools/make_store_media.py

The keys are drawn by the plugin's own painting code (the two generators above write them to scanner/target/demo),
and the game's panel next to them comes from the full screenshots in data/samples/ (not in git). Needs Pillow,
PyMuPDF (to draw the SVG keys) and, for the video, ffmpeg.
"""
import base64
import io
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import fitz
from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parent.parent
DEMO = ROOT / "scanner" / "target" / "demo"
SAMPLES = ROOT / "data" / "samples"
OUT = ROOT / "dist" / "store"

BG = (13, 14, 16)
BODY = (32, 32, 36)
YELLOW = (255, 231, 16)
CREAM = (240, 232, 214)
MUTED = (160, 156, 146)
FONT = "C:/Windows/Fonts/bahnschrift.ttf"
PANEL = (30, 30, 400, 600)  # the stratagem panel in a 2560x1440 screenshot (x0, y0, x1, y1)


def font(size: int, weight: str = "Regular") -> ImageFont.FreeTypeFont:
    f = ImageFont.truetype(FONT, size)
    try:
        f.set_variation_by_name(weight)
    except (OSError, ValueError):
        pass
    return f


def key_image(url: str, size: int, cache: dict) -> Image.Image:
    if (url, size) not in cache:
        head, data = url.split(",", 1)
        raw = base64.b64decode(data)
        if head.startswith("data:image/svg+xml"):
            page = fitz.open("svg", raw)[0]
            z = size / page.rect.width
            img = Image.open(io.BytesIO(page.get_pixmap(matrix=fitz.Matrix(z, z), alpha=False).tobytes("png")))
        else:
            img = Image.open(io.BytesIO(raw))
        cache[(url, size)] = img.convert("RGB").resize((size, size), Image.LANCZOS)
    return cache[(url, size)]


def paste_rounded(canvas: Image.Image, img: Image.Image, xy: tuple[int, int], radius: int) -> None:
    mask = Image.new("L", img.size, 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, img.width - 1, img.height - 1), radius=radius, fill=255)
    canvas.paste(img, xy, mask)


def deck(keys: list[str], key: int, cache: dict, cols: int = 5, pressed: int | None = None) -> Image.Image:
    """The keys laid out on a Stream Deck body."""
    gap, margin = key // 7, key // 4
    rows = -(-len(keys) // cols)
    w, h = 2 * margin + cols * key + (cols - 1) * gap, 2 * margin + rows * key + (rows - 1) * gap
    img = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    ImageDraw.Draw(img).rounded_rectangle((0, 0, w - 1, h - 1), radius=key // 4, fill=BODY + (255,))
    for i, url in enumerate(keys):
        x, y = margin + (i % cols) * (key + gap), margin + (i // cols) * (key + gap)
        k = key_image(url, key, cache)
        if pressed == i:
            s = int(key * 0.9)
            k, x, y = k.resize((s, s), Image.LANCZOS), x + (key - s) // 2, y + (key - s) // 2
        paste_rounded(img, k, (x, y), key // 7)
    return img


def backdrop(w: int, h: int, glow_at: tuple[int, int] | None = None) -> Image.Image:
    img = Image.new("RGB", (w, h), BG)
    if glow_at:
        glow = Image.new("RGB", (w, h), BG)
        r = min(w, h) // 2
        ImageDraw.Draw(glow).ellipse((glow_at[0] - r, glow_at[1] - r, glow_at[0] + r, glow_at[1] + r), fill=(48, 44, 14))
        img = Image.blend(img, glow.filter(ImageFilter.GaussianBlur(r // 2)), 1.0)
    return img


def shadowed(canvas: Image.Image, img: Image.Image, xy: tuple[int, int]) -> None:
    shadow = Image.new("RGBA", (img.width + 80, img.height + 80), (0, 0, 0, 0))
    ImageDraw.Draw(shadow).rounded_rectangle((40, 50, img.width + 40, img.height + 50), radius=30, fill=(0, 0, 0, 170))
    canvas.paste(shadow.filter(ImageFilter.GaussianBlur(22)), (xy[0] - 40, xy[1] - 40), shadow.filter(ImageFilter.GaussianBlur(22)))
    canvas.paste(img, xy, img if img.mode == "RGBA" else None)


def game_panel(sample: str, scale: float) -> Image.Image:
    shot = Image.open(SAMPLES / sample).convert("RGB").crop(PANEL)
    shot = shot.resize((int(shot.width * scale), int(shot.height * scale)), Image.LANCZOS)
    framed = Image.new("RGB", (shot.width + 8, shot.height + 8), (70, 68, 60))
    framed.paste(shot, (4, 4))
    return framed


def heading(img: Image.Image, title: str, sub: str) -> None:
    d = ImageDraw.Draw(img)
    d.rectangle((96, 84, 104, 156), fill=YELLOW)
    d.text((128, 76), title, font=font(58, "Bold"), fill=CREAM)
    d.text((130, 148), sub, font=font(30), fill=MUTED)


def arrow(img: Image.Image, x: int, y: int) -> None:
    d = ImageDraw.Draw(img)
    d.rectangle((x, y - 6, x + 46, y + 6), fill=YELLOW)
    d.polygon([(x + 40, y - 26), (x + 76, y), (x + 40, y + 26)], fill=YELLOW)


def make_icon() -> Image.Image:
    """The plugin's own icon (tools/icon_src/plugin.svg, an original drawing, no game artwork) at the Marketplace size."""
    page = fitz.open(str(ROOT / "tools" / "icon_src" / "plugin.svg"))[0]
    z = 288 / page.rect.width
    return Image.open(io.BytesIO(page.get_pixmap(matrix=fitz.Matrix(z, z), alpha=False).tobytes("png"))).convert("RGB")


def main() -> int:
    scenes = json.loads((DEMO / "scenes.json").read_text(encoding="utf-8"))
    frames = json.loads((DEMO / "frames.json").read_text(encoding="utf-8"))
    OUT.mkdir(parents=True, exist_ok=True)
    cache: dict = {}

    make_icon().save(OUT / "icon.png")

    # thumbnail: the name, three lines, and the keys of a real menu
    thumb = backdrop(1920, 960, (1380, 480))
    d = ImageDraw.Draw(thumb)
    d.text((112, 250), "STREAM DECK PLUGIN", font=font(34, "Bold"), fill=YELLOW)
    d.text((106, 300), "HELLDIVERS 2", font=font(118, "Bold"), fill=CREAM)
    d.text((106, 420), "STRATAGEMS", font=font(118, "Bold"), fill=CREAM)
    for i, line in enumerate(["Your loadout on your keys.", "Cooldowns from the game's own timer.", "One press to call in."]):
        d.text((112, 580 + i * 56), line, font=font(40), fill=MUTED)
    board = deck(scenes["scenes"]["menu_open"], 150, cache)
    shadowed(thumb, board, (1380 - board.width // 2, 480 - board.height // 2))
    thumb.save(OUT / "thumbnail.png")

    # gallery: the game's panel next to the keys the plugin draws for it
    pages = [
        ("gallery_1_loadout.png", "menu_open.png", "menu_open", "Your loadout, read from the game",
         "Each key shows the stratagem at its place in the menu, with the game's own counters."),
        ("gallery_2_cooldowns.png", "menu_unavailable.png", "menu_unavailable", "Cooldowns, call-ins, unavailable",
         "The keys fill like the game's icons and turn ready when the game's timer ends."),
    ]
    for name, sample, scene, title, sub in pages:
        img = backdrop(1920, 960, (1360, 560))
        heading(img, title, sub)
        panel = game_panel(sample, 1.25)
        shadowed(img, panel.convert("RGBA"), (130, 560 - panel.height // 2 + 20))
        arrow(img, 130 + panel.width + 60, 580)
        board = deck(scenes["scenes"][scene], 132, cache)
        shadowed(img, board, (1390 - board.width // 2, 580 - board.height // 2))
        img.save(OUT / name)

    # gallery: standby and the Scan key's looks
    img = backdrop(1920, 960, (960, 520))
    heading(img, "Waits for the game, starts it for you", "Nothing is captured until the game runs: under 1% of one CPU core. In a mission, a scan takes about 13 ms.")
    board = deck(scenes["scenes"]["standby"], 112, cache)
    shadowed(img, board, (130, 583 - board.height // 2))
    d = ImageDraw.Draw(img)
    looks = [("start", "Start", "launches the game through Steam"), ("missing", "Grey", "the game's window can't be seen"),
             ("lobby", "Yellow", "on the ship"), ("mission", "Green", "in a mission")]
    for i, (key, label, text) in enumerate(looks):
        x, y = 1010, 300 + i * 150
        paste_rounded(img, key_image(scenes["scan"][key], 116, cache), (x, y), 16)
        d.text((x + 150, y + 22), label, font=font(40, "Bold"), fill=CREAM)
        d.text((x + 150, y + 70), text, font=font(30), fill=MUTED)
    img.save(OUT / "gallery_3_standby.png")

    # gallery: a Stratagem Setup key through a press
    img = backdrop(1920, 960, (960, 560))
    heading(img, "Stratagem Setup: a whole drop in one press",
            "Two to four of your loadout stratagems on one key, called in and thrown one after the other.")
    d = ImageDraw.Draw(img)
    steps = [("ready", "Press", "the Quasar Cannon and the Warp Pack, both ready"),
             ("first", "Called in, thrown", "the Quasar Cannon is on its way"),
             ("both", "Then the next", "the Warp Pack follows, a throw later")]
    size, step = 240, 560
    for i, (key, label, text) in enumerate(steps):
        x = 960 - step + i * step - size // 2
        paste_rounded(img, key_image(scenes["setup"][key], size, cache), (x, 330), 30)
        d.text((x + size // 2, 610), label, font=font(42, "Bold"), fill=CREAM, anchor="mm")
        d.text((x + size // 2, 662), text, font=font(28), fill=MUTED, anchor="mm")
        if i < len(steps) - 1:
            arrow(img, x + size + 100, 450)
    d.text((960, 790), "Found in the menu wherever a mission puts them. A stratagem that isn't ready is skipped.",
           font=font(32), fill=MUTED, anchor="mm")
    img.save(OUT / "gallery_4_setup.png")

    # video: the scripted mission of the README, at 1920x1080
    if shutil.which("ffmpeg"):
        with tempfile.TemporaryDirectory() as tmp:
            n = 0
            for frame in frames["frames"]:
                img = backdrop(1920, 1080, (960, 600))
                d = ImageDraw.Draw(img)
                d.text((960, 120), "HELLDIVERS 2 STRATAGEMS", font=font(64, "Bold"), fill=CREAM, anchor="mm")
                d.text((960, 190), "for Stream Deck", font=font(36), fill=YELLOW, anchor="mm")
                board = deck(frame["keys"], 200, cache, pressed=frame["pressed"])
                shadowed(img, board, (960 - board.width // 2, 620 - board.height // 2))
                for _ in range(2):  # 200 ms a frame at 10 frames a second
                    img.save(Path(tmp) / f"f{n:04d}.png")
                    n += 1
            for _ in range(15):  # rest on the last frame
                img.save(Path(tmp) / f"f{n:04d}.png")
                n += 1
            subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-framerate", "10", "-i", str(Path(tmp) / "f%04d.png"),
                            "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "18", "-movflags", "+faststart",
                            str(OUT / "demo.mp4")], check=True)
    for f in sorted(OUT.iterdir()):
        print(f"{f.name}: {f.stat().st_size / 1024:.0f} KB")
    return 0


if __name__ == "__main__":
    sys.exit(main())
