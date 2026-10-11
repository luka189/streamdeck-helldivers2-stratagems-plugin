"""Check commit messages against Conventional Commits, as CONTRIBUTING.md describes them.

    python tools/commitlint.py --file .git/COMMIT_EDITMSG   # the commit-msg hook
    python tools/commitlint.py --range origin/dev..HEAD      # the commits of a branch
    python tools/commitlint.py --title "fix: stop the crash" # one title

Standard library only, so it runs in the git hook and in CI without installing anything.
Merge commits, reverts made by git and the fixup!/squash! commits of an autosquash are let through.
"""
import argparse
import re
import subprocess
import sys

TYPES = ("feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore", "revert")
MAX_TITLE = 100
TITLE = re.compile(r"^(?P<type>[a-z]+)(?:\((?P<scope>[a-z0-9][a-z0-9-]*)\))?(?P<bang>!)?: (?P<desc>.+)$")
EXEMPT = re.compile(r"^(Merge |Revert \"|fixup! |squash! |amend! )")


def check_title(title: str) -> list[str]:
    """Reasons the title breaks the rules; empty when it is fine."""
    if EXEMPT.match(title):
        return []
    m = TITLE.match(title)
    if not m:
        return ["the title must read `<type>(<scope>): <description>` (the scope is optional), for example `fix(engine): ...`"]
    errors = []
    if m["type"] not in TYPES:
        errors.append(f"`{m['type']}` is not a type; use one of {', '.join(TYPES)}")
    if len(title) > MAX_TITLE:
        errors.append(f"the title is {len(title)} characters long; keep it to {MAX_TITLE} and put the rest in the body")
    if title.endswith("."):
        errors.append("the title ends with a period")
    if m["desc"] != m["desc"].strip():
        errors.append("the description starts or ends with a space")
    return errors


def check_message(message: str) -> list[str]:
    lines = [line for line in message.splitlines() if not line.startswith("#")]
    while lines and not lines[0].strip():
        lines.pop(0)
    if not lines:
        return ["the message is empty"]
    errors = check_title(lines[0])
    if len(lines) > 1 and lines[1].strip() and not EXEMPT.match(lines[0]):
        errors.append("leave an empty line between the title and the body")
    if any(line.startswith("BREAKING CHANGE:") for line in lines[1:]) and "!:" not in lines[0]:
        errors.append("a BREAKING CHANGE footer needs a `!` before the colon of the title")
    return errors


def commits(rev_range: str) -> list[tuple[str, str]]:
    out = subprocess.run(["git", "log", "--format=%H%x00%B%x1e", rev_range], capture_output=True, text=True, encoding="utf-8", check=True)
    result = []
    for entry in out.stdout.split("\x1e"):
        if entry.strip():
            sha, _, body = entry.strip("\n").partition("\x00")
            result.append((sha, body))
    return result


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    group = ap.add_mutually_exclusive_group(required=True)
    group.add_argument("--file", help="a file holding one commit message (the commit-msg hook passes this)")
    group.add_argument("--range", help="a git revision range, for example origin/dev..HEAD")
    group.add_argument("--title", help="a single title, for example the title of a pull request")
    args = ap.parse_args()

    if args.title is not None:
        checked = [("title", check_title(args.title), args.title)]
    elif args.file is not None:
        with open(args.file, encoding="utf-8") as f:
            text = f.read()
        checked = [("message", check_message(text), text.splitlines()[0] if text else "")]
    else:
        checked = [(sha[:8], check_message(body), body.splitlines()[0] if body else "") for sha, body in commits(args.range)]

    bad = [(what, errors, title) for what, errors, title in checked if errors]
    for what, errors, title in bad:
        print(f"{what}: {title}", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
    if bad:
        print("See 'Commit messages' in CONTRIBUTING.md.", file=sys.stderr)
        return 1
    if args.range is not None:
        print(f"{len(checked)} commit(s) follow the rules")
    return 0


if __name__ == "__main__":
    sys.exit(main())
