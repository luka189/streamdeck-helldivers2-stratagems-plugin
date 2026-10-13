"""Record a play session, the game and the Stream Deck, and make a demo video of both.

    python tools/record_demo.py record [seconds]    # create dist/recording/stop to end it early
    python tools/record_demo.py make [--from S] [--to S] [--fast S S] [--speed X] [--offset MS]   # dist/recording/demo.mp4
    python tools/record_demo.py arm | disarm        # optional: the plugin records its keys too, from its next start

record finds the monitor the game is on and the Stream Deck window, which has to be visible on another monitor, and
records both with one ffmpeg (Desktop Duplication, NVENC, no mouse pointer) into dist/recording/game.mkv: the game as
its first video stream, the window as its second. make finds the keys in the window's picture and shows them over the
game, larger, in dist/recording/demo.mp4, from --from to --to seconds into the recording (all of it by default), and
plays the part between the two --fast times (seconds into the recording) --speed times faster (4 by default).

Without the window, make draws a Stream Deck from the keys the plugin recorded instead: after arm, the plugin writes
every image and title it sends to a key, and every press, to keys-record.jsonl in its folder from its next start; a
positive --offset shows those keys later. Needs ffmpeg with ddagrab and h264_nvenc, numpy, Pillow and PyMuPDF.
"""
import base64
import ctypes
import ctypes.wintypes as wt
import io
import json
import subprocess
import sys
import time
from pathlib import Path

import fitz
import numpy as np
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
PLUGIN = ROOT / "plugin" / "com.xiaodoudou.helldivers2-stratagems.sdPlugin"
FLAG, KEYS = PLUGIN / "record-keys", PLUGIN / "keys-record.jsonl"
OUT = ROOT / "dist" / "recording"
GAME, META, STOP, DEMO = OUT / "game.mkv", OUT / "game.json", OUT / "stop", OUT / "demo.mp4"
SCANNER = ROOT / "scanner" / "target" / "release" / "hd-scanner.exe"
FPS = 30
W, H = 1920, 1080  # the demo video
OVERLAY = (760, 390)  # largest size of the keys shown over the game
KEY, GAP, MARGIN, RADIUS = 104, 14, 22, 14  # the deck drawn from the plugin's record
DECK, BODY = (14, 14, 16), (32, 32, 36)
PRESSED, PRESS_MS = 0.9, 150  # a pressed key is drawn this much smaller, this long
FONT = "C:/Windows/Fonts/bahnschrift.ttf"

user32, kernel32 = ctypes.windll.user32, ctypes.windll.kernel32
user32.SetProcessDPIAware()


class MONITORINFO(ctypes.Structure):
    _fields_ = [("size", wt.DWORD), ("monitor", wt.RECT), ("work", wt.RECT), ("flags", wt.DWORD)]


def monitor_at(x: int, y: int) -> tuple[int, int, int, int]:
    """The rectangle (x, y, w, h) of the monitor showing the desktop point (x, y)."""
    user32.MonitorFromPoint.restype = ctypes.c_void_p
    mon = user32.MonitorFromPoint(wt.POINT(x, y), 2)
    info = MONITORINFO(); info.size = ctypes.sizeof(MONITORINFO)
    user32.GetMonitorInfoW(ctypes.c_void_p(mon), ctypes.byref(info))
    m = info.monitor
    return m.left, m.top, m.right - m.left, m.bottom - m.top


def process_window(exe: str) -> tuple[int, int, int, int] | None:
    """The rectangle of the largest visible, not minimised window of the process `exe`."""
    found = []

    def visit(h, _):
        if user32.IsWindowVisible(h) and not user32.IsIconic(h):
            pid = wt.DWORD(); user32.GetWindowThreadProcessId(h, ctypes.byref(pid))
            proc = kernel32.OpenProcess(0x1000, False, pid.value)
            if proc:
                buf, n = ctypes.create_unicode_buffer(260), wt.DWORD(260)
                if kernel32.QueryFullProcessImageNameW(proc, 0, buf, ctypes.byref(n)) and buf.value.lower().endswith("\\" + exe):
                    r = wt.RECT(); user32.GetWindowRect(h, ctypes.byref(r))
                    found.append((r.left, r.top, r.right - r.left, r.bottom - r.top))
                kernel32.CloseHandle(proc)
        return True

    user32.EnumWindows(ctypes.WINFUNCTYPE(ctypes.c_bool, wt.HWND, wt.LPARAM)(visit), 0)
    return max(found, key=lambda r: r[2] * r[3], default=None)


def small_grey(source: list[str]) -> np.ndarray:
    """One frame of an ffmpeg input, as a small grey picture to compare screens by."""
    vf = "hwdownload,format=bgra,scale=64:36" if source[-1].startswith("ddagrab") else "scale=64:36"
    raw = subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", *source, "-frames:v", "1", "-vf", vf, "-pix_fmt", "gray", "-f", "rawvideo", "-"],
                         capture_output=True, check=True).stdout
    return np.frombuffer(raw[:64 * 36], np.uint8).astype(int)


def capture_index(monitor: tuple[int, int, int, int]) -> int:
    """The Desktop Duplication output that shows `monitor`: the one whose picture is closest to a GDI grab of it."""
    x, y, w, h = monitor
    gdi = small_grey(["-f", "gdigrab", "-offset_x", str(x), "-offset_y", str(y), "-video_size", f"{w}x{h}", "-i", "desktop"])
    scores = []
    for idx in range(8):
        try:
            scores.append((np.abs(gdi - small_grey(["-f", "lavfi", "-i", f"ddagrab=output_idx={idx}:framerate=5"])).sum(), idx))
        except subprocess.CalledProcessError:
            break
    return min(scores)[1]


def record(seconds: float) -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    STOP.unlink(missing_ok=True)
    game = process_window("helldivers2.exe")
    game_mon = monitor_at(game[0] + game[2] // 2, game[1] + game[3] // 2) if game else monitor_at(0, 0)
    if not game:
        print("the game is not running: recording the primary monitor, where it should open")
    inputs = ["-f", "lavfi", "-i", f"ddagrab=output_idx={capture_index(game_mon)}:framerate={FPS}:draw_mouse=0"]
    meta = {"fps": FPS, "game_monitor": game_mon}
    deck = process_window("streamdeck.exe")
    if deck is None:
        print("the Stream Deck window is not open: recording the game only (make draws the keys the plugin recorded)")
    else:
        mon = monitor_at(deck[0] + deck[2] // 2, deck[1] + deck[3] // 2)
        if mon == game_mon:
            print("warning: the Stream Deck window is on the game's monitor; put it on the other one so the game does not cover it")
        # the window's part of its monitor, in even sizes for the encoder
        x0, y0 = max(deck[0], mon[0]) - mon[0], max(deck[1], mon[1]) - mon[1]
        x1, y1 = min(deck[0] + deck[2], mon[0] + mon[2]) - mon[0], min(deck[1] + deck[3], mon[1] + mon[3]) - mon[1]
        w, h = (x1 - x0) // 2 * 2, (y1 - y0) // 2 * 2
        inputs += ["-f", "lavfi", "-i", f"ddagrab=output_idx={capture_index(mon)}:framerate={FPS}:draw_mouse=0:offset_x={x0}:offset_y={y0}:video_size={w}x{h}"]
        meta["window"] = deck
        print(f"Stream Deck window {deck}: keep it where it is, and visible, while recording")
    maps = [a for i in range(len(inputs) // 4) for a in ("-map", f"{i}:v")]
    print(f"recording the game's monitor {game_mon} for up to {seconds:.0f} s into {GAME}")
    p = subprocess.Popen(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", *inputs, *maps, "-c:v", "h264_nvenc", "-preset", "p5", "-tune", "hq",
                          "-rc", "vbr", "-cq", "21", "-fps_mode", "cfr", "-r", str(FPS), "-t", str(seconds), "-progress", "pipe:1", "-nostats", str(GAME)],
                         stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
    for line in p.stdout:
        if "start_ms" not in meta and line.startswith("frame=") and int(line.split("=")[1]) > 0:
            meta["start_ms"] = (time.time() - int(line.split("=")[1]) / FPS) * 1000
            META.write_text(json.dumps(meta), encoding="utf-8")
        if STOP.exists():
            p.stdin.write("q"); p.stdin.flush()
            STOP.unlink(missing_ok=True)
    p.wait()
    print(f"{GAME}: {GAME.stat().st_size / 1e6:.0f} MB")
    return p.returncode


def key_grid(img: Image.Image) -> tuple[int, int, int, int]:
    """The box (x0, y0, x1, y1) around the keys in a picture of the Stream Deck window: in its left pane, between the
    header and the divider over the key's settings, the band of rows that differs most from the pane's background."""
    a = np.asarray(img.convert("RGB")).astype(int)
    h, w, _ = a.shape
    x_off = 14  # past the window's edge
    pane = a[:, x_off : int(w * 0.62) - 8]
    bg = np.median(pane[h // 4 : h // 2, 4:16].reshape(-1, 3), 0)
    diff = np.abs(pane - bg).sum(2) > 30
    share = diff.mean(1)
    divider = next((y for y in range(h // 3, h) if share[y] > 0.8), h)
    bands, start, last = [], None, None
    for y in range(40, divider):
        if diff[y].any():
            if start is None or y - last > 20:
                if start is not None:
                    bands.append((start, last))
                start = y
            last = y
    if start is not None:
        bands.append((start, last))
    y0, y1 = max(bands, key=lambda b: diff[b[0] : b[1] + 1].sum())
    xs = np.nonzero(diff[y0 : y1 + 1].any(0))[0]
    pad = 6
    return max(0, x_off + xs[0] - pad), max(0, y0 - pad), min(w, x_off + xs[-1] + pad + 1), min(h, y1 + pad + 1)


CUT: list[str] = []  # ffmpeg input options choosing the part of the recording to show (make --from / --to)
START = 0.0  # where that part starts, in seconds into the recording
FAST: tuple[float, float, int] | None = None  # (from, to, speed): a part played faster (make --fast / --speed)


def frames(stream: int, vf: str, size: tuple[int, int]):
    """The frames of one video stream of the recording, as raw RGB, at FPS."""
    p = subprocess.Popen(["ffmpeg", "-hide_banner", "-loglevel", "error", *CUT, "-i", str(GAME), "-map", f"0:v:{stream}", "-vf", f"{vf},fps={FPS}",
                          "-pix_fmt", "rgb24", "-f", "rawvideo", "-"], stdout=subprocess.PIPE)
    n = size[0] * size[1] * 3
    while len(buf := p.stdout.read(n)) == n:
        yield buf
    p.wait()


def panel(size: tuple[int, int]) -> tuple[Image.Image, Image.Image, int]:
    """A dark rounded panel to hold the keys, its mask, and its padding."""
    pad = 12
    w, h = size[0] + 2 * pad, size[1] + 2 * pad
    img = Image.new("RGB", (w, h), (20, 20, 22))
    mask = Image.new("L", (w, h), 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, w - 1, h - 1), radius=18, fill=235)
    return img, mask, pad


def shown(i: int) -> bool:
    """Whether frame `i` of the part shown goes into the video: inside the --fast part, one frame in --speed."""
    if FAST is None:
        return True
    t, (a, b, speed) = START + i / FPS, FAST
    return not a <= t < b or round((t - a) * FPS) % speed == 0


def fast_label(frame: Image.Image) -> None:
    """Mark a frame of the --fast part with its speed: "x4" and two play triangles, top right."""
    d = ImageDraw.Draw(frame, "RGBA")
    font = ImageFont.truetype(FONT, 34)
    text = f"x{FAST[2]}"
    tw = d.textbbox((0, 0), text, font=font)[2]
    w, h, x1, y0 = tw + 98, 56, W - 36, 36
    d.rounded_rectangle((x1 - w, y0, x1, y0 + h), radius=12, fill=(20, 20, 22, 215))
    d.text((x1 - w + 18, y0 + h / 2), text, font=font, fill=(255, 231, 16), anchor="lm")
    for k in range(2):
        x = x1 - 62 + k * 22
        d.polygon([(x, y0 + 16), (x + 20, y0 + h / 2), (x, y0 + h - 16)], fill=(255, 231, 16))


def in_fast(i: int) -> bool:
    return FAST is not None and FAST[0] <= START + i / FPS < FAST[1]


def make_from_window(streams: int) -> int:
    probe = subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-ss", "1", "-i", str(GAME), "-map", "0:v:1", "-frames:v", "1",
                            "-f", "image2pipe", "-vcodec", "png", "-"], capture_output=True, check=True).stdout
    x0, y0, x1, y1 = key_grid(Image.open(io.BytesIO(probe)))
    scale = min(OVERLAY[0] / (x1 - x0), OVERLAY[1] / (y1 - y0))
    ow, oh = int((x1 - x0) * scale) // 2 * 2, int((y1 - y0) * scale) // 2 * 2
    print(f"keys found at {x0},{y0} {x1 - x0}x{y1 - y0} in the window, shown at {ow}x{oh}")
    back, mask, pad = panel((ow, oh))
    at = (W - back.width - 36, H - back.height - 36)
    enc = encoder()
    n = 0
    for i, (game, keys) in enumerate(zip(frames(0, f"scale={W}:{H}:flags=lanczos", (W, H)),
                                         frames(1, f"crop={x1 - x0}:{y1 - y0}:{x0}:{y0},scale={ow}:{oh}:flags=lanczos", (ow, oh)))):
        if not shown(i):
            continue
        frame = Image.frombytes("RGB", (W, H), game)
        box = back.copy()
        box.paste(Image.frombytes("RGB", (ow, oh), keys), (pad, pad))
        frame.paste(box, at, mask)
        if in_fast(i):
            fast_label(frame)
        enc.stdin.write(frame.tobytes())
        n += 1
    enc.stdin.close(); enc.wait()
    print(f"{DEMO}: {n / FPS:.0f} s, {DEMO.stat().st_size / 1e6:.1f} MB")
    return 0


def encoder() -> subprocess.Popen:
    return subprocess.Popen(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", f"{W}x{H}", "-r", str(FPS),
                             "-i", "-", "-c:v", "h264_nvenc", "-preset", "p6", "-tune", "hq", "-rc", "vbr", "-cq", "22", "-pix_fmt", "yuv420p",
                             "-movflags", "+faststart", str(DEMO)], stdin=subprocess.PIPE)


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


def deck(state: dict, cols: int, rows: int, pressed: set, cache: dict) -> tuple[Image.Image, Image.Image]:
    """A Stream Deck showing the recorded keys, and its mask (rounded corners)."""
    w, h = 2 * MARGIN + cols * KEY + (cols - 1) * GAP, 2 * MARGIN + rows * KEY + (rows - 1) * GAP
    img = Image.new("RGB", (w, h), DECK)
    mask = Image.new("L", (w, h), 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, w - 1, h - 1), radius=28, fill=255)
    ImageDraw.Draw(img).rounded_rectangle((4, 4, w - 5, h - 5), radius=24, fill=BODY)
    font = ImageFont.truetype(FONT, 15)
    for (col, row), (url, title) in state.items():
        x, y = MARGIN + col * (KEY + GAP), MARGIN + row * (KEY + GAP)
        key = key_image(url, cache).copy() if url else Image.new("RGB", (KEY, KEY), (0, 0, 0))
        if title:
            ImageDraw.Draw(key).text((KEY // 2, KEY - 8), title, font=font, fill="white", anchor="ms", stroke_width=2, stroke_fill="black")
        if (col, row) in pressed:
            size = int(KEY * PRESSED)
            key = key.resize((size, size), Image.LANCZOS)
            x, y = x + (KEY - size) // 2, y + (KEY - size) // 2
        km = Image.new("L", key.size, 0)
        ImageDraw.Draw(km).rounded_rectangle((0, 0, key.width - 1, key.height - 1), radius=RADIUS * key.width // KEY, fill=255)
        img.paste(key, (x, y), km)
    return img, mask


def make_from_record(offset_ms: float, from_s: float) -> int:
    meta = json.loads(META.read_text(encoding="utf-8"))
    events = [e for e in map(json.loads, filter(str.strip, KEYS.read_text(encoding="utf-8").splitlines())) if e.get("key")]
    places = {tuple(e["key"]) for e in events}
    cols, rows = max(c for c, _ in places) + 1, max(r for _, r in places) + 1
    start = meta["start_ms"] + from_s * 1000 - offset_ms
    state: dict = {}
    presses: list = []
    i = 0

    def apply(e: dict) -> None:
        k = tuple(e["key"])
        url, title = state.get(k, ("", ""))
        if e["what"] == "image":
            state[k] = (e["value"], title)
        elif e["what"] == "title":
            state[k] = (url, e["value"])
        elif e["what"] == "press":
            presses.append((e["t"], k))

    while i < len(events) and events[i]["t"] <= start:  # what the keys showed when the recording began
        apply(events[i]); i += 1
    enc = encoder()
    cache: dict = {}
    drawn, shown = None, None
    n = 0
    for f, buf in enumerate(frames(0, f"scale={W}:{H}:flags=lanczos", (W, H))):
        t = start + f * 1000 / FPS
        while i < len(events) and events[i]["t"] <= t:
            apply(events[i]); i += 1
        pressed = {k for at, k in presses if t - at < PRESS_MS}
        now = (tuple(sorted(state.items())), tuple(sorted(pressed)))
        if now != shown:
            drawn, shown = deck(state, cols, rows, pressed, cache), now
        if not shown(f):
            continue
        frame = Image.frombytes("RGB", (W, H), buf)
        img, mask = drawn
        frame.paste(img, (W - img.width - 40, H - img.height - 40), mask)
        if in_fast(f):
            fast_label(frame)
        enc.stdin.write(frame.tobytes())
        n += 1
    enc.stdin.close(); enc.wait()
    print(f"{DEMO}: {n / FPS:.0f} s, {DEMO.stat().st_size / 1e6:.1f} MB")
    return 0


def main() -> int:
    cmd = sys.argv[1] if len(sys.argv) > 1 else ""
    if cmd == "arm":
        FLAG.touch()
        print(f"created {FLAG}: the plugin records its keys from its next start (restart Stream Deck)")
        return 0
    if cmd == "disarm":
        FLAG.unlink(missing_ok=True)
        print(f"removed {FLAG}")
        return 0
    if cmd == "record":
        return record(float(sys.argv[2]) if len(sys.argv) > 2 else 300)
    if cmd == "make":
        global START, FAST
        args = sys.argv[2:]
        if "--fast" in args:  # the only option with two values
            k = args.index("--fast")
            FAST = (float(args[k + 1]), float(args[k + 2]), 4)
            del args[k : k + 3]
        opts = dict(zip(args[::2], map(float, args[1::2])))
        if FAST and "--speed" in opts:
            FAST = (FAST[0], FAST[1], int(opts["--speed"]))
        if "--from" in opts:
            START = opts["--from"]
            CUT.extend(["-ss", str(opts["--from"])])
        if "--to" in opts:
            CUT.extend(["-to", str(opts["--to"])])
        streams = subprocess.run(["ffprobe", "-v", "error", "-select_streams", "v", "-show_entries", "stream=index", "-of", "csv=p=0", str(GAME)],
                                 capture_output=True, text=True, check=True).stdout.split()
        if len(streams) > 1:
            return make_from_window(len(streams))
        return make_from_record(opts.get("--offset", 0.0), opts.get("--from", 0.0))
    sys.exit(__doc__)


if __name__ == "__main__":
    sys.exit(main())
