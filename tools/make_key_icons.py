"""Frame the stratagem symbols of data/symbols/ the way the game does, into the key pictures of data/keys_144/.

    python tools/make_key_icons.py

Each key picture is 144x144, named after its stratagem (several share one game icon but not one symbol): a border in the colour of the stratagem's type, taken from the game's icon in
data/icons_144/ (gold mission, red offensive, blue supply, green defensive), a dark inside of the same tint, and the
symbol (a 126x126 SVG, the inside of the game's icon) on top. Stratagems without a symbol of their own borrow the one
of a stratagem the game draws with the same icon; one with neither keeps its icon from data/icons_144/ (the plugin
falls back to it when data/keys_144/ has no picture).
"""
import io
import json
import re
import sys
from pathlib import Path

import fitz
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SYMBOLS = ROOT / "data" / "symbols"
ICONS = ROOT / "data" / "icons_144"
OUT = ROOT / "data" / "keys_144"
SIZE, BORDER, INNER = 144, 9, 126

# frame colour of the game's icon -> inside colour (measured on data/icons_144)
TINTS = {(85, 185, 210): (1, 20, 25), (201, 178, 105): (54, 52, 38), (220, 100, 85): (25, 3, 1), (105, 150, 85): (8, 25, 1)}

# symbol files whose names differ from the plugin's stratagem names
MANUAL = {
    "CQC-20 Breaching Hammer": "CQC-20",
    "EAT-411 Leveller": "EAT-411",
    "Eagle Gas Airstrike": "Eagle Gas Strike",
    "M-102 Gunner FRV": "Fast Recon Vehicle",
    "GL-28 Belt-Fed Grenade Launcher": "GL-28",
    "AX/TX-13 Dog Breath": "Guard Dog Breath",
    "AX/FLAM-75 Hot Dog": "Guard Dog Hot Dog",
    "AX/ARC-3 K-9": "Guard Dog K-9",
    "AX/LAS-5 Rover": "Guard Dog Rover",
    "B-100 Portable Hellbomb": "Hellbomb Portable",
}
# stratagems drawn by the game with the icon of another (see `hd-scanner lookalikes`), which have no symbol of their own
TWINS = {
    "SSSD Delivery": "Upload Data",
    "Reinforcement Pods": "Upload Data",
    "Activate E-711 Extraction Drill": "Prospecting Drill",
    "Aquifer Drill": "Prospecting Drill",
    "Portable Comms Relay": "Cargo Container",
}


def slug(name: str) -> str:
    """The key picture's file name for a stratagem; scanner's Stratagem::key_path makes the same."""
    return re.sub(r"[^0-9A-Za-z]+", "_", name).strip("_") + ".png"


def norm(s: str) -> str:
    s = s.replace("’", "'").lower()
    s = re.sub(r"\bmines?\b", "mine", s)
    return re.sub(r"[^0-9a-z]+", " ", s).strip()


def without_designation(name: str) -> str:
    """'LAS-99 Quasar Cannon' -> 'Quasar Cannon' (orbital and Eagle names have none)."""
    first, _, rest = name.partition(" ")
    return rest if rest and re.search(r"[0-9/-]", first) and not name.startswith(("Eagle", "Orbital")) else name


def symbol_for(name: str, symbols: dict[str, Path]) -> Path | None:
    if name in MANUAL:
        return symbols.get(norm(MANUAL[name]))
    for key in (norm(name), norm(without_designation(name))):
        if key in symbols:
            return symbols[key]
    return None


def plain_attributes(svg: str) -> str:
    """Turn style="fill:...;fill-rule:..." into attributes: the renderer ignores fill-rule given as CSS, which fills the
    holes of shapes drawn with evenodd (the gas mask of Dog Breath)."""
    def convert(m: re.Match) -> str:
        props = [p.split(":", 1) for p in m.group(1).split(";") if ":" in p]
        return " ".join(f'{k.strip()}="{v.strip()}"' for k, v in props)
    return re.sub(r'style="([^"]*)"', convert, svg)


def key_picture(symbol: Path, frame: tuple[int, int, int]) -> Image.Image:
    img = Image.new("RGBA", (SIZE, SIZE), frame + (255,))
    img.paste(TINTS[frame] + (255,), (BORDER, BORDER, SIZE - BORDER, SIZE - BORDER))
    page = fitz.open(stream=plain_attributes(symbol.read_text(encoding="utf-8")).encode(), filetype="svg")[0]
    z = INNER * 4 / page.rect.width  # drawn at 4x, then scaled down for clean edges
    glyph = Image.open(io.BytesIO(page.get_pixmap(matrix=fitz.Matrix(z, z), alpha=True).tobytes("png"))).convert("RGBA")
    img.alpha_composite(glyph.resize((INNER, INNER), Image.LANCZOS), (BORDER, BORDER))
    return img


def main() -> int:
    catalog = json.loads((ROOT / "data" / "stratagems.json").read_text(encoding="utf-8"))
    symbols = {}
    for f in sorted(SYMBOLS.glob("*.svg")):
        symbols.setdefault(norm(f.stem), f)
        symbols.setdefault(norm(without_designation(f.stem)), f)
    OUT.mkdir(parents=True, exist_ok=True)
    for old in OUT.glob("*.png"):
        old.unlink()
    by_name = {s["name"]: s for s in catalog}
    made, borrowed, kept = 0, [], []
    for s in catalog:
        name = s["name"]
        symbol = symbol_for(name, symbols)
        if symbol is None and name in TWINS:
            symbol = symbol_for(TWINS[name], symbols)
            borrowed.append(f"{name} <- {TWINS[name]}")
        icon = ICONS / (Path(s["icon"]).stem + ".png")
        if symbol is None:
            kept.append(name)
            continue
        frame = Image.open(icon).convert("RGB").getpixel((4, 72))
        if frame not in TINTS:
            raise SystemExit(f"{name}: unknown frame colour {frame}")
        key_picture(symbol, frame).save(OUT / slug(name), optimize=True)
        made += 1
    print(f"{made} key pictures in {OUT.relative_to(ROOT)}")
    print("borrowed:", "; ".join(borrowed))
    print("game icon kept (no symbol):", ", ".join(kept) or "none")
    unused = sorted({f.stem for f in SYMBOLS.glob("*.svg")} - {symbol_for(n, symbols).stem for n in by_name if symbol_for(n, symbols)})
    if unused:
        print("symbols not used:", ", ".join(unused))
    return 0




def page(out: Path) -> None:
    """An HTML page with every stratagem's game icon next to its key picture, for review."""
    import base64

    catalog = json.loads((ROOT / "data" / "stratagems.json").read_text(encoding="utf-8"))
    groups = {(201, 178, 105): "Mission (gold)", (220, 100, 85): "Offensive (red)", (85, 185, 210): "Supply (blue)", (105, 150, 85): "Defensive (green)"}

    def uri(path: Path) -> str:
        return "data:image/png;base64," + base64.b64encode(path.read_bytes()).decode()

    cards = {k: [] for k in groups}
    for s in catalog:
        icon = ICONS / (Path(s["icon"]).stem + ".png")
        key = OUT / slug(s["name"])
        frame = Image.open(icon).convert("RGB").getpixel((4, 72))
        key_html = f'<img src="{uri(key)}">' if key.exists() else '<div class="todo">game icon kept</div>'
        cards[frame].append(f'<div class="card"><div class="pair"><figure><img src="{uri(icon)}"><figcaption>game</figcaption></figure>'
                            f'<figure>{key_html}<figcaption>key</figcaption></figure></div><div class="name">{s["name"]}</div></div>')
    sections = "".join(f'<h2>{groups[k]} <span>{len(v)}</span></h2><div class="grid">{"".join(v)}</div>' for k, v in cards.items())
    out.write_text(f"""<!doctype html><html><head><meta charset="utf-8"><title>Key icons</title><style>
body {{ margin:0; padding:20px 28px 40px; background:#1b1b1d; color:#e8e4da; font:14px system-ui, sans-serif; }}
h1 {{ margin:0; font-size:22px; }} .sub {{ color:#a8a49a; margin:4px 0 14px; }}
h2 {{ font-size:16px; color:#ffe710; margin:24px 0 10px; }} h2 span {{ color:#a8a49a; font-weight:400; }}
.bar {{ position:sticky; top:0; background:#1b1b1d; padding:8px 0; z-index:1; }}
.grid {{ display:flex; flex-wrap:wrap; gap:12px; }} .card {{ background:#2a2a2d; border-radius:10px; padding:10px; }}
.pair {{ display:flex; gap:8px; }} figure {{ margin:0; text-align:center; }} figcaption {{ font-size:11px; color:#8f8b82; margin-top:4px; }}
.name {{ font-size:12px; margin-top:6px; max-width:300px; }}
img, .todo {{ width:144px; height:144px; display:block; }} body.keys img, body.keys .todo {{ width:72px; height:72px; }}
.todo {{ display:flex; align-items:center; justify-content:center; border:1px dashed #555; color:#777; font-size:12px; box-sizing:border-box; }}
</style></head><body><h1>Key icons: game icon and key picture</h1>
<div class="sub">The key pictures frame the symbols of nvigneux/Helldivers-2-Stratagems-icons-svg the way the game does.</div>
<div class="bar"><label><input type="checkbox" id="keys"> Key size (72 px)</label></div>{sections}
<script>document.getElementById('keys').onchange = e => document.body.classList.toggle('keys', e.target.checked);</script></body></html>""",
                   encoding="utf-8")
    print(out)


if __name__ == "__main__":
    if "--page" in sys.argv:
        page(Path(sys.argv[sys.argv.index("--page") + 1]))
    else:
        sys.exit(main())
