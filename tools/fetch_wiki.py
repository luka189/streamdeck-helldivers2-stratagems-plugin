"""Download the stratagem list (name, code, cooldown) and icons from helldivers.wiki.gg."""
import html
import json
import re
import sys
import urllib.request
from pathlib import Path

BASE = "https://helldivers.wiki.gg"
ROOT = Path(__file__).resolve().parent.parent
DATA = ROOT / "data"
ICONS = DATA / "icons_svg"
ARROW = {"Up": "U", "Down": "D", "Left": "L", "Right": "R"}


def get(url: str) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": "streamdeck-helldivers/0.1"})
    with urllib.request.urlopen(req, timeout=30) as r:
        return r.read()


def main() -> int:
    ICONS.mkdir(parents=True, exist_ok=True)
    page = json.loads(get(f"{BASE}/api.php?action=parse&page=Stratagems&prop=text&format=json"))
    text = page["parse"]["text"]["*"]

    out = []
    seen = set()
    for row in re.findall(r"<tr>(.*?)</tr>", text, flags=re.S):
        cells = re.findall(r"<td[^>]*>(.*?)</td>", row, flags=re.S)
        if len(cells) < 4:
            continue
        icon = re.search(r'src="(/images/[^"?]*Stratagem_Icon[^"?]*\.svg)', cells[0])
        name = re.sub(r"<[^>]+>", "", cells[1]).strip()
        arrows = re.findall(r'alt="Stratagem Arrow (\w+)\.svg"', cells[2])
        if not icon or not name or not arrows or name in seen:
            continue
        seen.add(name)
        cd = re.sub(r"<[^>]+>", "", cells[3]).strip()
        out.append({
            "name": html.unescape(name),
            "code": "".join(ARROW[a] for a in arrows),
            "cooldown": cd,
            "icon": Path(icon.group(1)).name,
        })

    (DATA / "stratagems.json").write_text(json.dumps(out, indent=2), encoding="utf-8")
    print(f"{len(out)} stratagems")

    total = 0
    for s in out:
        dest = ICONS / s["icon"]
        if dest.exists():
            continue
        blob = get(f"{BASE}/images/{s['icon']}")
        dest.write_bytes(blob)
        total += len(blob)
        print("icon", s["icon"], len(blob))
    print(f"downloaded {total} bytes")
    return 0


if __name__ == "__main__":
    sys.exit(main())
