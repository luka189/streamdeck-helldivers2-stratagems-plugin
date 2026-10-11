"""Render the downloaded SVG icons to PNG at the sizes the matcher and the deck keys use."""
import re
import sys
from pathlib import Path

import fitz

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "data" / "icons_svg"
SIZES = {"icons_144": 144}


def inline_css(text: str) -> str:
    """PyMuPDF ignores <style> classes (CorelDRAW exports), so copy each rule onto its elements."""
    rules = {m.group(1): m.group(2) for m in re.finditer(r"\.([\w-]+)\s*\{([^}]*)\}", text)}
    if not rules:
        return text
    return re.sub(
        r'class="([^"]*)"',
        lambda m: 'style="' + ";".join(rules.get(c, "") for c in m.group(1).split()) + '"',
        text,
    )


def render(svg: Path, size: int, dest: Path) -> None:
    data = inline_css(svg.read_text(encoding="utf-8")).encode("utf-8")
    page = fitz.open(stream=data, filetype="svg")[0]
    zoom = size / max(page.rect.width, page.rect.height)
    page.get_pixmap(matrix=fitz.Matrix(zoom, zoom), alpha=True).save(str(dest))


def main() -> int:
    for folder in SIZES:
        (ROOT / "data" / folder).mkdir(parents=True, exist_ok=True)
    count = 0
    for svg in sorted(SRC.glob("*.svg")):
        for folder, size in SIZES.items():
            render(svg, size, ROOT / "data" / folder / (svg.stem + ".png"))
        count += 1
    print(f"converted {count}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
