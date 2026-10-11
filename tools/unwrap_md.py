"""Join hard-wrapped Markdown paragraphs and list items into single lines, in place.

    python tools/unwrap_md.py README.md docs/*.md

Some Markdown viewers render every line break, so the documentation keeps one paragraph per line. Code blocks, tables, headings
and images are left alone.
"""
import re
import sys
from pathlib import Path

ITEM = re.compile(r"^(\s*)([-*+]|\d+\.)\s")
BLOCK = re.compile(r"^\s*(\||#|!\[|<|>|---\s*$)")


def unwrap(text: str) -> str:
    out: list[str] = []
    fence = False
    joinable = False  # the last output line is prose that a following prose line continues
    for line in text.splitlines():
        if line.lstrip().startswith("```"):
            fence = not fence
            out.append(line)
            joinable = False
            continue
        if fence or not line.strip() or BLOCK.match(line):
            out.append(line)
            joinable = False
            continue
        if ITEM.match(line):
            out.append(line.rstrip())
            joinable = True
            continue
        if joinable:
            out[-1] = out[-1].rstrip() + " " + line.strip()
        else:
            out.append(line.rstrip())
            joinable = True
    return "\n".join(out) + "\n"


def main() -> int:
    for name in sys.argv[1:]:
        p = Path(name)
        before = p.read_text(encoding="utf-8")
        after = unwrap(before)
        if after != before:
            p.write_text(after, encoding="utf-8")
            print(f"unwrapped {p}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
